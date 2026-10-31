//! Home, as the engine sends it: the spotlight and the sections, flattened into the one list the
//! cursor walks.
//!
//! The engine decides which sections there are and what is in them. This keeps the whole
//! [`HomeView`], and mirrors its titles into [`App::content`] in the order they are drawn, so the
//! cursor, the preview and every key that acts on "the highlighted title" work as on any list.

use anistream_core::ids::AnilistId;
use anistream_i18n::t;
use anistream_model::{
    catalog::{ContentList, Entry},
    command::Command,
    home::{
        HomeAction, HomeItem, HomeSection, HomeView, Playable, SectionKind, Spotlight,
        SpotlightCase,
    },
};

use super::{App, alert, info};
use crate::nav::{Section, StageView};

/// One row of the Home list: the spotlight, or an item of a section.
#[derive(Clone, Copy)]
pub(crate) enum HomeRow<'a> {
    Spotlight(&'a Spotlight),
    Item(&'a HomeSection, &'a HomeItem),
}

impl HomeRow<'_> {
    pub(crate) fn entry(&self) -> &Entry {
        match self {
            Self::Spotlight(s) => &s.entry,
            Self::Item(_, item) => &item.entry,
        }
    }

    /// The episode the row's main action plays, when it plays one.
    fn plays(&self) -> Option<u32> {
        let (action, episode) = match self {
            Self::Spotlight(s) => (s.action, Some(s.episode)),
            Self::Item(_, item) => (item.action, item.episode),
        };
        matches!(action, HomeAction::Resume | HomeAction::Play).then_some(episode).flatten()
    }

    /// The title is the viewer's own Continue, which Not interested does not apply to.
    fn continuing(&self) -> bool {
        match self {
            Self::Spotlight(s) => s.case == SpotlightCase::Continuing,
            Self::Item(section, _) => section.kind == SectionKind::Continue,
        }
    }

    /// A show the viewer follows, in the week's schedule.
    fn followed(&self) -> bool {
        matches!(self, Self::Item(section, _) if section.kind == SectionKind::ThisWeek)
    }
}

/// The rows of `view` in the order they are drawn: the spotlight, then each section's items.
pub(crate) fn rows(view: &HomeView) -> Vec<HomeRow<'_>> {
    view.spotlight
        .iter()
        .map(HomeRow::Spotlight)
        .chain(
            view.sections.iter().flat_map(|section| {
                section.items.iter().map(move |i| HomeRow::Item(section, i))
            }),
        )
        .collect()
}

impl App {
    /// Take the Home the engine sent.
    pub(super) fn apply_home(&mut self, view: HomeView) {
        if view.failed {
            self.push_toast(alert(t::home_failed()));
        }
        self.home = Some(view);
        self.stamp_playable();
        self.sync_home();
        self.probe_home_selection();
    }

    /// Mirror the titles of Home into the list on screen, when Home is the section on screen.
    pub(super) fn sync_home(&mut self) {
        if self.nav.section() != Section::Home {
            return;
        }
        let Some(view) = &self.home else { return };
        let entries: Vec<Entry> = rows(view).iter().map(|row| row.entry().clone()).collect();
        if entries.is_empty() {
            // Nothing yet, and more on its way, is still loading; nothing and settled is the
            // empty state, or the offline one.
            self.content = if !view.pending.is_empty() {
                ContentList::Loading
            } else if view.offline {
                ContentList::Failed(t::home_offline_empty_body())
            } else {
                ContentList::Empty
            };
            return;
        }
        let cursor = self.cursors.section_mut(Section::Home);
        cursor.selected = cursor.selected.min(entries.len() - 1);
        cursor.offset = cursor.offset.min(cursor.selected);
        self.content = ContentList::Entries(entries);
    }

    /// The Home row under the cursor, while Home's list is what is on screen.
    pub(crate) fn home_row(&self) -> Option<HomeRow<'_>> {
        let at_home = self.nav.section() == Section::Home
            && matches!(self.nav.current(), StageView::Section(Section::Home));
        if !at_home {
            return None;
        }
        let view = self.home.as_ref()?;
        rows(view).into_iter().nth(self.cursors.home.selected)
    }

    /// The cursor is on something `x` hides as Not interested: a discovery title, not the
    /// viewer's own Continue or their week.
    pub fn on_discovery(&self) -> bool {
        self.home_row().is_some_and(|row| !row.continuing() && !row.followed())
    }

    /// The episode Enter plays on the highlighted row, when it has a main action that plays.
    pub(super) fn home_plays(&self) -> Option<(anistream_core::ids::AnilistId, u32)> {
        let row = self.home_row()?;
        row.plays().map(|episode| (row.entry().id, episode))
    }

    /// Hide the highlighted Home title: off Continue, or Not interested for a discovery title.
    /// Dropped from the screen at once; the engine sends Home again once the write is done.
    pub(super) fn hide_from_home(&mut self) -> Option<Command> {
        let row = self.home_row()?;
        let (id, title) = (row.entry().id, row.entry().title.clone());
        let upcoming = matches!(row, HomeRow::Item(_, item) if item.starts.is_some())
            || matches!(row, HomeRow::Spotlight(s) if s.starts.is_some());
        let (command, said) = if row.continuing() {
            (Command::HideFromContinue { id }, t::lists_continue_removed(&title))
        } else if row.followed() {
            self.push_toast(info(t::lists_remove_wrong_list()));
            return None;
        } else if upcoming {
            (Command::DismissUpNext { id }, t::up_next_hidden_until_airs(&title))
        } else {
            (Command::DismissUpNext { id }, t::up_next_hidden(&title))
        };
        self.last_dismissed = matches!(command, Command::DismissUpNext { .. }).then_some(id);
        if let Some(view) = &mut self.home {
            // A discovery title leaves every shelf it is on; a Continue row leaves Continue.
            let only_continue = matches!(command, Command::HideFromContinue { .. });
            for section in &mut view.sections {
                if only_continue == (section.kind == SectionKind::Continue) {
                    let before = section.items.len();
                    section.items.retain(|item| item.entry.id != id);
                    section.total =
                        section.total.saturating_sub((before - section.items.len()) as u32);
                }
            }
            view.sections.retain(|section| !section.items.is_empty());
            if view.spotlight.as_ref().is_some_and(|s| s.entry.id == id) {
                view.spotlight = None;
            }
        }
        self.sync_home();
        self.push_toast(info(said));
        Some(command)
    }

    /// Ask whether a source has the highlighted Home title, once as the cursor rests on it. The
    /// engine keeps the answer for a day and asks no more than once a minute.
    pub(super) fn probe_home_selection(&mut self) {
        let Some(id) = self.home_row().map(|row| row.entry().id) else { return };
        if self.last_probed != Some(id) {
            self.last_probed = Some(id);
            self.commands.push(Command::ProbeTitle { id });
        }
    }

    /// Tab on Home: the first row of the next group (the spotlight, then each section), round to
    /// the first. `false` when Home has only one group, so Tab keeps its other job.
    pub(super) fn jump_home_section(&mut self) -> bool {
        let Some(view) = &self.home else { return false };
        let mut starts: Vec<usize> = Vec::new();
        let mut at = 0;
        if view.spotlight.is_some() {
            starts.push(0);
            at = 1;
        }
        for section in &view.sections {
            if !section.items.is_empty() {
                starts.push(at);
                at += section.items.len();
            }
        }
        if starts.len() < 2 || self.home_row().is_none() {
            return false;
        }
        let selected = self.cursors.home.selected;
        let next = starts.iter().copied().find(|start| *start > selected).unwrap_or(starts[0]);
        let cursor = self.cursors.section_mut(Section::Home);
        cursor.selected = next;
        cursor.offset = next;
        self.probe_home_selection();
        true
    }

    /// `u` on Home: bring back the title last hidden with Not interested.
    pub(super) fn undo_home_dismissal(&mut self) -> Option<Command> {
        let id = self.last_dismissed.take()?;
        Some(Command::RestoreUpNext { id })
    }

    /// Whether Home's list is on screen with the stage focused: where Tab and `u` mean Home's.
    pub(super) fn on_home_stage(&self) -> bool {
        self.home_row().is_some() && self.nav.focus() == crate::nav::Focus::Stage
    }

    /// Keep the sources check's answer for `id` and show it on Home.
    pub(super) fn apply_playable(&mut self, id: AnilistId, playable: Playable) {
        self.playable.insert(id, playable);
        self.stamp_playable();
    }

    fn stamp_playable(&mut self) {
        let Some(view) = &mut self.home else { return };
        let known = &self.playable;
        let get = |id: AnilistId| known.get(&id).copied();
        if let Some(spotlight) = &mut view.spotlight
            && let Some(p) = get(spotlight.entry.id)
        {
            spotlight.playable = p;
        }
        for item in view.sections.iter_mut().flat_map(|s| s.items.iter_mut()) {
            if let Some(p) = get(item.entry.id) {
                item.playable = p;
            }
        }
    }
}
