//! A title's page: the title every title action means, the pages a relation was opened
//! from, and the pickers and lists that ride its record.

use anistream_core::ids::AnilistId;
use anistream_model::{
    catalog::{ContentList, Entry, LibrarySegment},
    command::Command,
    episodes::EpisodeRow,
    notice::Notice,
};

use super::App;
use crate::nav::{Overlay, StageView};

/// A title page left for one of its relations or recommendations.
///
/// Opening a related title replaces `detail` and, once its episodes are asked for, the episode
/// table; Back pops the view but nothing put them back, so the parent's page and table came back
/// showing the child. The frame keeps what the parent's view was showing, keyed by the stage
/// depth of that view so a frame can only ever be restored into the view it was taken from.
#[derive(Debug, Clone)]
pub(super) struct TitleFrame {
    depth: usize,
    entry: Entry,
    episodes_all: Vec<EpisodeRow>,
    /// Whose rows `episodes_all` are, put back with them. Left behind, the parent's rows came
    /// back owned by the child, whose rows were the last loaded: the parent's progress then
    /// found no row to land on, and the child's was written over the parent's episodes.
    episodes_for: Option<AnilistId>,
    episode_selected: usize,
    provider_preference: Option<String>,
}

impl App {
    /// The title the screen on top is about: the one every title action means.
    ///
    /// On a list it is the highlighted row — as the fuller record of the same title when that is
    /// the one last opened, because relations, recommendations and the trailer only ride the
    /// detail fetch. On a title page or its episode table it is that title, and on Now Playing
    /// the title playing, where the app holds its record. Anywhere else it is nothing, and the
    /// action says so rather than reaching for a title off screen.
    ///
    /// Reading `detail` first, as the handlers used to, acted on whichever title was opened
    /// last: back on the list, `a`, `*`, Space and the rest all went to that show rather than
    /// the highlighted one.
    pub fn target(&self) -> Option<&Entry> {
        match self.nav.current() {
            StageView::Title(id) | StageView::Episodes(id) => {
                self.detail.as_ref().filter(|d| d.id == *id)
            }
            StageView::Section(_) => {
                let row = self.selected_entry()?;
                Some(self.detail.as_ref().filter(|d| d.id == row.id).unwrap_or(row))
            }
            StageView::NowPlaying => {
                let id = self.playing.as_ref()?.id?;
                self.detail.iter().chain(self.content.entries()).find(|e| e.id == id)
            }
            StageView::Stores | StageView::Store(_) | StageView::StoreMembers(_) => None,
        }
    }

    /// "… from the title's page", when the screen is not one — said for what rides the detail
    /// fetch and came back empty here, as `m` and `P` say it for what only that page can do.
    ///
    /// A list row never opened carries no relations, recommendations or trailer, so "none known
    /// for this title" off the page was a guess that read as a fact.
    fn off_the_page(&self, what: &str) -> Option<String> {
        self.title_on_screen().is_none().then(|| format!("{what} from the title's page"))
    }

    /// The title whose page or episode table is on top, by the view's own id.
    pub(super) fn title_on_screen(&self) -> Option<AnilistId> {
        match self.nav.current() {
            StageView::Title(id) | StageView::Episodes(id) => Some(*id),
            _ => None,
        }
    }

    /// The title here, on AniList in a browser.
    pub(super) fn open_in_browser(&mut self) -> Option<Command> {
        let Some(id) = self.target().map(|e| e.id) else {
            self.push_toast(Notice::info("nothing selected"));
            return None;
        };
        Some(Command::OpenExternal { url: format!("https://anilist.co/anime/{}", id.get()) })
    }

    /// The source picker for the title on screen, opened on the choice in force.
    pub(super) fn pick_provider(&mut self) -> Option<Command> {
        // The choice already in force arrives with the title's record, so only a title's
        // own page knows it. From a list the overlay would open on the pin of whichever
        // title was opened last, and confirming "automatic" there could quietly undo a
        // pin the user made.
        if self.title_on_screen().is_none() {
            let hint = if self.target().is_some() {
                "a source is pinned from the title's page"
            } else {
                "nothing selected"
            };
            self.push_toast(Notice::info(hint));
            return None;
        }
        if self.providers.is_empty() {
            // Plugins register a moment after launch, so "none yet" is a real state
            // rather than a misconfiguration.
            self.push_toast(Notice::info("no sources registered yet"));
            return None;
        }
        // Start on the current choice, so the overlay opens showing where you are.
        self.cursors.overlay.selected = self
            .provider_choices()
            .iter()
            .position(|(id, _)| id.as_deref() == self.provider_preference.as_deref())
            .unwrap_or(0);
        self.nav.open_overlay(Overlay::SourceProvider);
        None
    }

    /// The source picked, pinned for the title on screen.
    pub(super) fn confirm_source_provider(&mut self) -> Option<Command> {
        let (provider, _) = self.provider_choices().get(self.cursors.overlay.selected)?.clone();
        let id = self.title_on_screen()?;
        self.nav.close_overlay();
        self.cursors.overlay.selected = 0;
        // Reflected immediately rather than waiting for the write to come back: the
        // overlay has closed, and a row that still showed the old choice would read
        // as the keystroke having missed.
        self.provider_preference = provider.clone();
        Some(Command::SetProviderPreference { id, provider })
    }

    /// The title's trailer, when its record has one.
    pub(super) fn play_trailer(&mut self) -> Option<Command> {
        let Some(entry) = self.target() else {
            self.push_toast(Notice::info("nothing selected"));
            return None;
        };
        let Some(url) = entry.trailer_url.clone() else {
            // Rides the detail fetch, so a list row answers after the title has
            // been opened once — and some titles genuinely have no trailer.
            let why = self
                .off_the_page("the trailer is found")
                .unwrap_or_else(|| "no trailer on record for this title".into());
            self.push_toast(Notice::info(why));
            return None;
        };
        Some(Command::PlayTrailer { url, title: entry.title.clone() })
    }

    /// What AniList recommends after the title here, when its record says.
    pub(super) fn show_recommendations(&mut self) -> Option<Command> {
        let Some(entry) = self.target() else {
            self.push_toast(Notice::info("nothing selected"));
            return None;
        };
        if entry.recommended.is_empty() {
            // Like relations, recommendations ride the detail fetch — and some
            // titles genuinely have none.
            let why = self
                .off_the_page("recommendations are listed")
                .unwrap_or_else(|| "no recommendations known for this title".into());
            self.push_toast(Notice::info(why));
            return None;
        }
        self.cursors.overlay.selected = 0;
        self.nav.open_overlay(Overlay::Recommendations);
        None
    }

    /// Into the recommended title picked, with this page kept to come back to.
    pub(super) fn confirm_recommendation(&mut self) -> Option<Command> {
        let pick = self.target()?.recommended.get(self.cursors.overlay.selected)?.clone();
        self.nav.close_overlay();
        self.cursors.overlay.selected = 0;
        self.leave_title();
        // Same stand-in rule as the watch order: never render one title's data
        // under another while the fetch is in flight.
        self.detail = Some(Entry::new(pick.id, pick.title));
        self.nav.push(StageView::Title(pick.id));
        Some(Command::LoadDetail(pick.id))
    }

    /// The titles connected to the one here, when its record says.
    pub(super) fn show_watch_order(&mut self) -> Option<Command> {
        let Some(entry) = self.target() else {
            self.push_toast(Notice::info("nothing selected"));
            return None;
        };
        if entry.related.is_empty() {
            // Empty on list rows too — the relations ride the detail fetch. From a
            // list this still answers after opening the title once.
            let why = self
                .off_the_page("connected seasons are listed")
                .unwrap_or_else(|| "no connected seasons known for this title".into());
            self.push_toast(Notice::info(why));
            return None;
        }
        self.cursors.overlay.selected = 0;
        self.nav.open_overlay(Overlay::WatchOrder);
        None
    }

    /// Into the connected title picked, with this page kept to come back to.
    pub(super) fn confirm_watch_order(&mut self) -> Option<Command> {
        let related = self.target()?.related.get(self.cursors.overlay.selected)?.clone();
        self.nav.close_overlay();
        self.cursors.overlay.selected = 0;
        // Kept, so Back shows this page again rather than the one about to open.
        self.leave_title();
        // A minimal entry stands in until the detail fetch answers — the same
        // stale-data rule as episodes: never render one title's data under another.
        self.detail = Some(Entry::new(related.id, related.title));
        self.nav.push(StageView::Title(related.id));
        Some(Command::LoadDetail(related.id))
    }

    /// The list status picker for the title here, opened on the status it has.
    pub(super) fn pick_list_status(&mut self) {
        if self.signed_in_trackers().is_empty() {
            // Nothing keeps a list status but a tracker. Offering the picker anyway
            // ended in "marked watching" for a choice that went nowhere.
            self.push_toast(Notice::info(
                "list status is kept on a tracker, and none is signed in — :accounts",
            ));
        } else if let Some(entry) = self.target() {
            // Preselect what the title is already on, so the common case is one
            // keystroke to confirm rather than a hunt. The title's own status, where it
            // is known — the Library tab last viewed said nothing about a show opened
            // from search, and preselected "dropped" for it. Unknown starts at the top.
            let current = self.statuses_set.get(&entry.id).copied().or(entry.list_status);
            self.cursors.overlay.selected = current
                .and_then(|status| LibrarySegment::ALL.iter().position(|s| *s == status))
                .unwrap_or(0);
            self.nav.open_overlay(Overlay::ListStatus);
        } else {
            self.push_toast(Notice::info("nothing selected"));
        }
    }

    /// The rating picker for the title here.
    pub(super) fn pick_rating(&mut self) {
        if self.signed_in_trackers().is_empty() {
            self.push_toast(Notice::info(
                "ratings are kept on a tracker, and none is signed in — :accounts",
            ));
        } else if self.target().is_some() {
            // No local record of an existing score to preselect, so the cursor
            // starts at the top and every row is one or two keystrokes away.
            self.cursors.overlay.selected = 0;
            self.nav.open_overlay(Overlay::Rate);
        } else {
            self.push_toast(Notice::info("nothing selected"));
        }
    }

    /// The list status picked for the title here, and on every copy of it on screen.
    pub(super) fn confirm_list_status(&mut self) -> Option<Command> {
        let entry = self.target()?;
        let id = entry.id;
        let status = *LibrarySegment::ALL.get(self.cursors.overlay.selected)?;
        self.nav.close_overlay();
        self.push_toast(Notice::info(format!(
            "now on your {} list — queued for {}",
            status.label(),
            self.signed_in_trackers().join(", ")
        )));
        // Remembered on every copy of the title on screen, and past the next reload,
        // so asking again opens on the answer just given.
        self.statuses_set.insert(id, status);
        let entries = match &mut self.content {
            ContentList::Entries(entries) => entries.as_mut_slice(),
            _ => &mut [],
        };
        for entry in self.detail.iter_mut().chain(entries.iter_mut()) {
            if entry.id == id {
                entry.list_status = Some(status);
            }
        }
        Some(Command::SetStatus { id, status })
    }

    /// The score picked for the title here.
    pub(super) fn confirm_rating(&mut self) -> Option<Command> {
        let entry = self.target()?;
        let id = entry.id;
        // Rows run 10 down to 1, so the row index is the distance from 10.
        let score = (10 - self.cursors.overlay.selected.min(9)) as f32;
        self.nav.close_overlay();
        self.push_toast(Notice::info(format!(
            "rated {score}/10 — queued for {}",
            self.signed_in_trackers().join(", ")
        )));
        Some(Command::SetScore { id, score })
    }

    /// The title page's record, asked for again.
    pub(super) fn refresh_title(&mut self) -> Option<Command> {
        self.images.requests().retry_failed();
        self.title_on_screen().map(Command::LoadDetail)
    }

    /// Remember the title page on screen before a relation or recommendation replaces it.
    fn leave_title(&mut self) {
        let Some(entry) = self.title_on_screen().and_then(|_| self.detail.clone()) else {
            return;
        };
        let depth = self.nav.depth();
        // Anything at this depth or deeper describes views that have since been popped.
        self.title_trail.retain(|frame| frame.depth < depth);
        self.title_trail.push(TitleFrame {
            depth,
            entry,
            episodes_all: self.episodes_all.clone(),
            episodes_for: self.episodes_for,
            episode_selected: self.cursors.episodes.selected,
            provider_preference: self.provider_preference.clone(),
        });
    }

    /// After a pop, show the title page a relation was opened from as it was, when that page
    /// is where Back landed.
    pub(super) fn restore_title(&mut self) {
        let depth = self.nav.depth();
        self.title_trail.retain(|frame| frame.depth <= depth);
        let Some(here) = self.title_on_screen() else { return };
        if !self.title_trail.last().is_some_and(|frame| frame.depth == depth) {
            return;
        }
        let Some(frame) = self.title_trail.pop().filter(|frame| frame.entry.id == here) else {
            return;
        };
        self.detail = Some(frame.entry);
        self.episodes_all = frame.episodes_all;
        self.episodes_for = frame.episodes_for;
        // Cleared first so the filter places the cursor from the remembered row, not from
        // whichever row of the child's table it was on.
        self.episodes.clear();
        self.cursors.episodes.selected = frame.episode_selected;
        self.apply_episode_filter();
        self.episodes_loading = false;
        self.provider_preference = frame.provider_preference;
    }

    /// A title's full record, in place of the stand-in set as it was opened.
    pub(super) fn apply_detail(&mut self, mut entry: Box<Entry>) {
        if let Some(detail) = self.detail.as_ref().filter(|d| d.id == entry.id) {
            // The record does not carry the tracker's status; the row it was opened
            // from may, and losing it here would make `a` forget what it is on.
            entry.list_status = entry.list_status.or(detail.list_status);
            self.detail = Some(*entry);
        }
    }

    /// What the source picker offers: automatic first, then every registered source.
    ///
    /// `None` is automatic — the ordinary `providers.order` chain with its failover. It leads
    /// because it is the right answer for almost every title, and pinning is the exception.
    ///
    /// Sources that are registered but currently unusable are still listed. Health is a moment's
    /// state, and hiding a source that is briefly failing would make the choice look impossible
    /// rather than temporarily unavailable — the row says so instead.
    pub fn provider_choices(&self) -> Vec<(Option<String>, String)> {
        let mut choices = vec![(None, "Automatic — best available source".to_string())];
        choices.extend(self.providers.iter().map(|row| {
            let mut label = row.id.clone();
            if !row.kind.is_empty() {
                label.push_str(&format!("  ·  {}", row.kind));
            }
            if row.held_back {
                label.push_str("  ·  held back");
            } else if !row.usable {
                label.push_str("  ·  failing");
            }
            (Some(row.id.clone()), label)
        }));
        choices
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::fixtures::*;

    #[test]
    fn a_list_status_is_confirmed_as_the_list_it_is_on() {
        let mut a = app();
        // List status lives on a tracker, so one is signed in.
        a.apply(Event::Sync(Box::new(connected(0))));
        a.apply(a.content_here(entries(1)));
        a.handle(Action::SetListStatus, 10);
        a.set_overlay_cursor(2);
        let task = a.handle(Action::Open, 10);
        assert!(matches!(
            task,
            Some(Command::SetStatus { status: LibrarySegment::Completed, .. })
        ));
        assert!(
            a.toasts
                .iter()
                .any(|t| t.notice.text == "now on your completed list — queued for anilist"),
            "{:?}",
            a.toasts
        );
    }

    #[test]
    fn setting_a_list_status_preselects_the_current_one() {
        // One keystroke to confirm in the common case, rather than a hunt.
        let mut a = app();
        a.apply(Event::Sync(Box::new(connected(0))));
        a.go_to_section(Section::Library);
        a.library_segment = LibrarySegment::Completed;
        let mut rows = entries(3);
        if let ContentList::Entries(rows) = &mut rows {
            // As the Library's own fetch marks its rows.
            rows.iter_mut().for_each(|row| row.list_status = Some(LibrarySegment::Completed));
        }
        a.apply(a.content_here(rows));
        a.handle(Action::SetListStatus, 20);
        assert_eq!(a.nav.overlay(), Some(&Overlay::ListStatus));
        assert_eq!(a.overlay_cursor(), 2);

        let task = a.handle(Action::Open, 20);
        assert_eq!(
            task,
            Some(Command::SetStatus {
                id: AnilistId::new(1),
                status: LibrarySegment::Completed
            })
        );
        assert!(!a.nav.has_overlay());
    }

    #[test]
    fn a_status_given_this_session_outlives_a_reload_until_the_tracker_answers() {
        let mut a = app();
        a.apply(Event::Sync(Box::new(connected(0))));
        a.go_to_section(Section::Home);
        a.apply(a.content_here(entries(3)));
        let planning = 1;
        a.handle(Action::SetListStatus, 20);
        a.handle(Action::Down, 20);
        assert_eq!(
            a.handle(Action::Open, 20),
            Some(Command::SetStatus {
                id: AnilistId::new(1),
                status: LibrarySegment::Planning
            })
        );

        // Home again: the rows are rebuilt and carry no status of their own.
        a.go_to_section(Section::Home);
        a.apply(a.content_here(entries(3)));
        a.handle(Action::SetListStatus, 20);
        assert_eq!(a.overlay_cursor(), planning, "the status given before the reload");
        a.handle(Action::Back, 20);

        // A pull that lists the title is the tracker's answer, and wins.
        a.go_to_section(Section::Library);
        a.library_segment = LibrarySegment::Completed;
        let mut rows = entries(3);
        if let ContentList::Entries(rows) = &mut rows {
            rows.iter_mut().for_each(|row| row.list_status = Some(LibrarySegment::Completed));
        }
        a.apply(a.content_here(rows));
        a.go_to_section(Section::Home);
        a.apply(a.content_here(entries(3)));
        a.handle(Action::SetListStatus, 20);
        assert_eq!(a.overlay_cursor(), 0, "nothing given since the pull, so nothing known");
    }

    #[test]
    fn rating_a_title_queues_a_score_out_of_ten() {
        let mut a = app();
        a.apply(Event::Sync(Box::new(connected(0))));
        a.apply(a.content_here(entries(3)));
        a.handle(Action::RateTitle, 20);
        assert_eq!(a.nav.overlay(), Some(&Overlay::Rate));

        // Rows run 10 down to 1, so two steps down from the top is an 8.
        a.handle(Action::Down, 20);
        a.handle(Action::Down, 20);
        let task = a.handle(Action::Open, 20);
        assert_eq!(task, Some(Command::SetScore { id: AnilistId::new(1), score: 8.0 }));
        assert!(!a.nav.has_overlay());
    }

    #[test]
    fn recommendations_open_a_list_and_enter_walks_into_the_pick() {
        let mut a = app();
        a.apply(a.content_here(entries(1)));
        let mut detail = Entry::new(AnilistId::new(1), "Hoshikuzu");
        detail.recommended = vec![RelatedTitle {
            id: AnilistId::new(99),
            title: "Kumo no Ue no Toshokan".into(),
            relation: String::new(),
            format: Some("TV".into()),
        }];
        a.detail = Some(detail);

        a.handle(Action::Recommendations, 20);
        assert_eq!(a.nav.overlay(), Some(&Overlay::Recommendations));
        let task = a.handle(Action::Open, 20);
        assert_eq!(task, Some(Command::LoadDetail(AnilistId::new(99))));
        assert!(matches!(a.nav.current(), StageView::Title(id) if id.get() == 99));
    }

    #[test]
    fn the_trailer_key_plays_when_known_and_says_so_when_not() {
        let mut a = app();
        a.apply(a.content_here(entries(1)));

        // A list row never opened: the trailer is on the record its page fetches.
        a.handle(Action::PlayTrailer, 20);
        assert!(a.toasts.iter().any(|t| t.notice.text.contains("from the title's page")));

        let mut detail = Entry::new(AnilistId::new(1), "Hoshikuzu");
        detail.trailer_url = Some("https://www.youtube.com/watch?v=x".into());
        a.detail = Some(detail);
        let task = a.handle(Action::PlayTrailer, 20);
        assert_eq!(
            task,
            Some(Command::PlayTrailer {
                url: "https://www.youtube.com/watch?v=x".into(),
                title: "Hoshikuzu".into(),
            })
        );
    }

    #[test]
    fn recommendations_with_none_known_say_so_rather_than_opening_nothing() {
        let mut a = app();
        a.apply(a.content_here(entries(1)));
        a.handle(Action::Recommendations, 20);
        assert!(!a.nav.has_overlay());
        assert!(a.toasts.iter().any(|t| t.notice.text.contains("from the title's page")));
    }

    #[test]
    fn status_and_rating_with_no_tracker_signed_in_say_so_instead_of_pretending() {
        // "marked watching" and "rated 8/10" used to be said for choices that went nowhere.
        // A tracker that is configured but signed out does not count.
        let mut a = app();
        a.apply(Event::Sync(Box::new(SyncState::new("kitsu"))));
        a.apply(a.content_here(entries(3)));

        for action in [Action::SetListStatus, Action::RateTitle] {
            a.toasts.clear();
            assert_eq!(a.handle(action, 20), None);
            assert!(!a.nav.has_overlay(), "{action:?} offered a picker that goes nowhere");
            let said = &a.toasts.last().expect("nothing said").notice.text;
            assert!(said.contains("none is signed in") && said.contains(":accounts"), "{said}");
        }
    }

    #[test]
    fn a_status_says_which_trackers_it_was_queued_for() {
        let mut a = app();
        a.apply(Event::Sync(Box::new(connected(0))));
        a.apply(Event::Sync(Box::new(SyncState { connected: true, ..SyncState::new("mal") })));
        a.apply(Event::Sync(Box::new(SyncState::new("kitsu"))));
        a.apply(a.content_here(entries(3)));
        a.handle(Action::SetListStatus, 20);
        a.handle(Action::Open, 20);
        let said = &a.toasts.last().unwrap().notice.text;
        assert!(said.ends_with("queued for anilist, mal"), "{said}");
    }

    #[test]
    fn rating_with_nothing_selected_says_so() {
        let mut a = app();
        a.apply(Event::Sync(Box::new(connected(0))));
        assert_eq!(a.handle(Action::RateTitle, 20), None);
        assert!(!a.nav.has_overlay());
        assert!(a.toasts.iter().any(|t| t.notice.text.contains("nothing selected")));
    }

    #[test]
    fn a_title_page_leaves_the_list_under_it_where_it_was() {
        // Every movement key on a title page used to move the list hidden behind it, so Back
        // landed rows away from the title just looked at.
        let mut a = home_list(8);
        a.handle(Action::Down, 5);
        a.handle(Action::Down, 5);
        assert_eq!(a.handle(Action::Open, 5), Some(Command::LoadDetail(AnilistId::new(3))));
        for action in [
            Action::Down,
            Action::Down,
            Action::Up,
            Action::Bottom,
            Action::Top,
            Action::PageDown,
            Action::PageUp,
            Action::Bottom,
        ] {
            assert_eq!(a.handle(action, 5), None, "{action:?}");
        }
        assert_eq!((a.cursor(), a.offset()), (2, 0), "the hidden list moved");
        assert_eq!(a.target().map(|e| e.id), Some(AnilistId::new(3)));

        a.handle(Action::Back, 5);
        assert_eq!(a.nav.current(), &StageView::Section(Section::Home));
        assert_eq!(a.selected_entry().map(|e| e.id), Some(AnilistId::new(3)));
    }

    #[test]
    fn a_list_rows_actions_go_to_that_row_not_the_title_opened_last() {
        // Back on a list, every title key read the last title opened before the highlighted
        // row, so `a`, `*`, `d`, Space and the rest all went to the wrong show.
        let mut a = home_list(5);
        // List status lives on a tracker, so one is signed in.
        a.apply(Event::Sync(Box::new(connected(0))));
        a.handle(Action::Open, 5);
        // The record the title page fetched, with everything that rides that fetch.
        let mut opened = Entry::new(AnilistId::new(1), "Title 0");
        opened.trailer_url = Some("https://video.example/trailer".into());
        opened.related = vec![related(90, "Its sequel")];
        opened.recommended = vec![related(91, "Something like it")];
        a.apply(Event::Detail(Box::new(opened)));
        a.handle(Action::Back, 5);
        a.handle(Action::Down, 5);
        a.handle(Action::Down, 5);
        let row = AnilistId::new(3);
        let first = || "1".to_string();

        assert_eq!(
            a.handle(Action::Download, 5),
            Some(Command::DownloadEpisode { id: row, episode: first() })
        );
        assert_eq!(
            a.handle(Action::ShowSources, 5),
            Some(Command::LoadSources { id: row, episode: first() })
        );
        assert_eq!(
            a.handle(Action::PlayParty, 5),
            Some(Command::PlayParty { id: row, episode: first() })
        );
        assert_eq!(
            a.handle(Action::OpenInBrowser, 5),
            Some(Command::OpenExternal { url: "https://anilist.co/anime/3".into() })
        );

        a.handle(Action::SetListStatus, 5);
        assert_eq!(
            a.handle(Action::Open, 5),
            Some(Command::SetStatus { id: row, status: LibrarySegment::Watching })
        );
        a.handle(Action::RateTitle, 5);
        assert_eq!(a.handle(Action::Open, 5), Some(Command::SetScore { id: row, score: 10.0 }));

        // What rides the detail fetch is the opened title's, not this row's.
        assert_eq!(a.handle(Action::PlayTrailer, 5), None);
        for action in [Action::WatchOrder, Action::Recommendations] {
            a.handle(action, 5);
            assert_eq!(a.nav.overlay(), None, "{action:?} offered the opened title's list");
        }
        // Pinning a source and fixing a match need the title's own page; from a list they
        // say so instead of opening on the other title's state.
        for action in [Action::PickProvider, Action::FixMapping] {
            assert_eq!(a.handle(action, 5), None);
            assert_eq!(a.nav.overlay(), None, "{action:?}");
        }

        assert_eq!(a.handle(Action::ShowEpisodes, 5), Some(Command::LoadEpisodes(row)));
        a.handle(Action::Back, 5);
        assert_eq!(
            a.handle(Action::PlayNext, 5),
            Some(Command::Play { id: row, episode: first() })
        );
        assert_eq!(a.playing.as_ref().map(|p| p.title.as_str()), Some("Title 2"));
    }

    #[test]
    fn back_from_a_related_title_shows_the_page_it_was_opened_from() {
        let mut a = home_list(3);
        a.handle(Action::Open, 5);
        let parent = AnilistId::new(1);
        let mut full = Entry::new(parent, "Title 0");
        full.synopsis = "What the parent is about.".into();
        full.related = vec![related(90, "Its sequel")];
        a.apply(Event::Detail(Box::new(full.clone())));

        a.handle(Action::WatchOrder, 5);
        assert_eq!(a.handle(Action::Open, 5), Some(Command::LoadDetail(AnilistId::new(90))));
        assert_eq!(a.target().map(|e| e.id), Some(AnilistId::new(90)));

        a.handle(Action::Back, 5);
        assert_eq!(a.nav.current(), &StageView::Title(parent));
        assert_eq!(a.target(), Some(&full), "the parent's page came back showing the child");
    }

    #[test]
    fn back_from_a_related_title_gives_the_parents_rows_back_to_the_parent() {
        // The trail put the parent's rows back but left them owned by the child, whose rows
        // were the last loaded: the engine's word on a mark made on them found no row to land
        // on, and the child's own progress would be written over the parent's episodes.
        let mut a = app_at_episodes();
        let (parent, child) = (AnilistId::new(1), AnilistId::new(90));
        a.detail.as_mut().expect("the parent's record").related =
            vec![related(90, "Its sequel")];
        a.handle(Action::WatchOrder, 20);
        assert_eq!(a.handle(Action::Open, 20), Some(Command::LoadDetail(child)));
        assert_eq!(a.handle(Action::ShowEpisodes, 20), Some(Command::LoadEpisodes(child)));
        let row = |n: &str| EpisodeRow {
            number: n.into(),
            title: None,
            duration_secs: None,
            watched: 0.0,
            completed: false,
            kind: None,
            skippable: false,
            thumbnail: None,
            description: None,
        };
        a.apply(Event::Episodes { id: child, rows: vec![row("1"), row("2")] });
        assert_eq!(a.episodes_for, Some(child));

        a.handle(Action::Back, 20);
        a.handle(Action::Back, 20);
        assert_eq!(a.nav.current(), &StageView::Episodes(parent));
        assert_eq!(a.episodes.len(), 12, "the parent's rows are back");
        assert_eq!(a.episodes_for, Some(parent), "but still owned by the child");
        assert!(a.episode_rows_for(child).is_empty(), "the parent's rows spoke for the child");

        let said = |id, number: &str| Event::Progress {
            id,
            progress: None,
            resume: None,
            episodes: vec![anistream_model::progress::EpisodeProgress {
                number: number.into(),
                watched: 1.0,
                completed: true,
            }],
        };
        // The child's progress is not the parent's, however its episodes are numbered.
        a.apply(said(child, "2"));
        assert!(!a.episodes[1].completed, "the child's progress was written over the parent's");

        // A mark made here is the parent's, and the engine's answer lands on its row.
        assert_eq!(
            a.handle(Action::ToggleWatched, 20),
            Some(Command::SetWatched { id: parent, episodes: vec!["1".into()], watched: true })
        );
        a.apply(said(parent, "1"));
        assert!(a.episodes[0].completed, "the parent's answer found no row to land on");
    }

    #[test]
    fn the_status_picker_opens_on_the_titles_status_not_the_tab_last_viewed() {
        let mut a = home_list(3);
        // List status lives on a tracker, so one is signed in.
        a.apply(Event::Sync(Box::new(connected(0))));
        a.library_segment = LibrarySegment::Dropped;
        a.handle(Action::SetListStatus, 5);
        assert_eq!(a.overlay_cursor(), 0, "nothing known about it, so the top");
        a.set_overlay_cursor(3);
        assert_eq!(
            a.handle(Action::Open, 5),
            Some(Command::SetStatus { id: AnilistId::new(1), status: LibrarySegment::Paused })
        );
        a.handle(Action::SetListStatus, 5);
        assert_eq!(a.overlay_cursor(), 3, "the answer just given");
    }

    #[test]
    fn what_rides_the_detail_fetch_is_pointed_at_the_title_page_from_a_list() {
        // A row never opened has no relations, recommendations or trailer to go on, so
        // "none known for this title" there was a guess. `m` and `P` already say where to go.
        let mut a = home_list(3);
        let last = |a: &App| a.toasts.last().map(|t| t.notice.text.clone()).unwrap_or_default();
        for (action, said) in [
            (Action::WatchOrder, "connected seasons are listed from the title's page"),
            (Action::Recommendations, "recommendations are listed from the title's page"),
            (Action::PlayTrailer, "the trailer is found from the title's page"),
        ] {
            assert_eq!(a.handle(action, 5), None);
            assert_eq!(last(&a), said);
        }
        // On the page itself, empty is the answer.
        a.handle(Action::Open, 5);
        a.handle(Action::WatchOrder, 5);
        assert_eq!(last(&a), "no connected seasons known for this title");
    }

    #[test]
    fn left_on_a_title_goes_back_rather_than_stranding_it_behind_the_rail() {
        // `h` focused the rail over the title page, and the next `j` switched section and
        // threw the whole stack away.
        let mut a = home_list(5);
        a.handle(Action::Open, 5);
        a.handle(Action::Left, 5);
        assert_eq!(a.nav.current(), &StageView::Section(Section::Home));
        assert_eq!(a.nav.focus(), Focus::Stage);
        a.handle(Action::Down, 5);
        assert_eq!((a.nav.section(), a.cursor()), (Section::Home, 1));
        a.handle(Action::Left, 5);
        assert_eq!(a.nav.focus(), Focus::Rail);
    }

    #[test]
    fn a_late_record_for_a_title_already_left_is_not_shown_on_the_next() {
        let mut a = app();
        a.apply(a.content_here(entries(3)));
        a.nav.focus_stage();
        a.handle(Action::Open, 20); // title 1
        a.handle(Action::Back, 20);
        a.handle(Action::Down, 20);
        a.handle(Action::Open, 20); // title 2
        a.apply(Event::Detail(Box::new(Entry::new(AnilistId::new(1), "Title one, in full"))));
        assert_eq!(a.detail.as_ref().map(|d| d.id), Some(AnilistId::new(2)));

        a.apply(Event::Detail(Box::new(Entry::new(AnilistId::new(2), "Title two, in full"))));
        assert_eq!(a.detail.as_ref().map(|d| d.title.as_str()), Some("Title two, in full"));
    }

    #[test]
    fn watch_order_says_so_when_there_is_nothing_to_show() {
        // Silence would read as the key being broken.
        let mut a = app();
        a.handle(Action::WatchOrder, 10);
        assert_eq!(a.toasts.len(), 1);
        assert!(a.toasts[0].notice.text.contains("nothing selected"));
    }

    #[test]
    fn watch_order_lists_relations_and_a_pick_opens_the_title() {
        let mut a = app_at_episodes();
        if let Some(detail) = a.detail.as_mut() {
            detail.related = vec![RelatedTitle {
                id: AnilistId::new(9),
                title: "Season 2".into(),
                relation: "sequel".into(),
                format: Some("TV".into()),
            }];
        }
        a.handle(Action::WatchOrder, 10);
        assert_eq!(a.nav.overlay(), Some(&Overlay::WatchOrder));

        let task = a.handle(Action::Open, 10).expect("a pick must open the title");
        assert_eq!(task, Command::LoadDetail(AnilistId::new(9)));
        assert_eq!(a.nav.overlay(), None);
        assert!(
            matches!(a.nav.current(), StageView::Title(id) if *id == AnilistId::new(9)),
            "the picked title's view must be up"
        );
    }
}
