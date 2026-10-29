//! Calendar: the week, Monday first, as seven columns on wide windows and as an agenda below
//! that. The week is in the header bar's title with the year under it, previous and next beside
//! it, and My List or All Airing in a toggle group under the header bar.

use std::{
    cell::{Cell, RefCell},
    rc::Rc,
    time::Instant,
};

use adw::prelude::*;
use anistream_core::settings::SettingValue;
use anistream_i18n::t;
use anistream_model::{
    catalog::{ContentFor, ContentList, Entry, LibrarySegment},
    command::Command,
    event::Event,
    present,
    settings::SettingKey,
};
use gtk::{gio, glib};

use crate::{
    fit::Fit,
    motion,
    states::{self, Shape, States},
    widgets,
    window::{Ctx, Size},
};

/// One broadcast: a title's episode and when it airs, in Unix seconds.
#[derive(Debug, Clone)]
struct Broadcast {
    entry: Entry,
    episode: u32,
    airs: i64,
    /// When it was to air before the schedule moved it.
    was: Option<i64>,
}

impl Broadcast {
    fn watched(&self) -> bool {
        self.entry.progress.as_ref().is_some_and(|p| p.done >= self.episode)
    }

    /// Aired and not watched: the water dot.
    fn is_new(&self, now: i64) -> bool {
        self.airs <= now && !self.watched()
    }

    /// The air time, with "~" before an estimate: "~16:30".
    fn clock(&self) -> String {
        present::air_time(&time(self.airs), self.entry.airing_approximate)
    }

    /// "Dub" when the row is the dub's: a sub is the plain case and has no mark.
    fn dub(&self) -> bool {
        present::calendar_mark(&self.entry) == Some("Dub") // i18n-skip
    }

    /// "Ep 10 · 16:30 · Dub": "~16:30" for an estimate, and the mark only on a dub. One line,
    /// which never wraps; where the broadcast moved from is in [`Broadcast::moved`].
    fn line(&self) -> String {
        let episode = i64::from(self.episode);
        if self.dub() {
            t::calendar_row_time_dubbed(episode, &self.clock())
        } else {
            t::calendar_row_time(episode, &self.clock())
        }
    }

    /// "Moved from 23:00", when the schedule moved it.
    fn moved(&self) -> Option<String> {
        self.was.map(|was| t::calendar_moved_from(&time(was)))
    }

    /// The line as a screen reader says it: "Episode 10, about 16:30, dub, moved from 23:00".
    fn spoken(&self) -> String {
        let at = time(self.airs);
        let mut said = vec![t::episode_number(i64::from(self.episode))];
        said.push(if self.entry.airing_approximate {
            t::calendar_linux_about(&at)
        } else {
            at
        });
        if self.dub() {
            said.push(t::calendar_linux_dubbed());
        }
        if let Some(was) = self.was {
            said.push(t::calendar_moved_from(&time(was)));
        }
        said.join(", ")
    }
}

/// Whether a title is yours: under way here, or on your Watching or Planning list.
fn is_mine(entry: &Entry) -> bool {
    entry.progress.is_some()
        || matches!(
            entry.list_status,
            Some(LibrarySegment::Watching | LibrarySegment::Planning)
        )
}

struct Page {
    ctx: Rc<Ctx>,
    me: RefCell<std::rc::Weak<Page>>,
    rows: RefCell<Vec<Entry>>,
    /// What the rows' times count from, how far back and ahead the calendar reaches, and how to
    /// ask for more of either side.
    anchor: Cell<Option<i64>>,
    floor: Cell<Option<i64>>,
    ceiling: Cell<Option<i64>>,
    pages: Cell<u32>,
    more_past: Cell<bool>,
    asking_past: Cell<bool>,
    failed_past: Cell<bool>,
    ahead_pages: Cell<u32>,
    more_ahead: Cell<bool>,
    asking_ahead: Cell<bool>,
    failed_ahead: Cell<bool>,
    /// Weeks from this one.
    week: Cell<i32>,
    mine: Cell<bool>,
    /// Set while the toggle follows the setting, so that is not a pick.
    syncing: Cell<bool>,
    title: adw::WindowTitle,
    today: gtk::Button,
    previous: gtk::Button,
    next: gtk::Button,
    toggles: adw::ToggleGroup,
    body: Rc<States>,
    /// The week's columns or the agenda, whichever the width takes.
    views: gtk::Stack,
    /// The week's columns live on two grids that take turns; a step slides to the other.
    week_stack: gtk::Stack,
    grids: [gtk::Grid; 2],
    grid_on: Cell<usize>,
    /// The week (and its look) on the visible grid, to choose how the next one arrives.
    shown_week: Cell<Option<i32>>,
    look: Cell<Look>,
    /// The week whose broadcasts are awaited, its placeholder timer, and when they appeared.
    waiting: Cell<Option<i32>>,
    wait_timer: RefCell<Option<glib::SourceId>>,
    since: Cell<Option<Instant>>,
    agenda: gtk::Box,
    empty: adw::StatusPage,
    loaded: Cell<bool>,
    /// Whether the week on screen came from an answer, so stepping weeks before one, or after
    /// a failure, leaves the state alone.
    answered: Cell<bool>,
}

/// Where a day stands against the days the calendar reaches.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Reach {
    Before,
    In,
    After,
}

/// What a week's columns hold: broadcasts, nothing yet, or placeholders for them.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Look {
    Cards,
    Empty,
    Placeholders,
}

/// What the loaded broadcasts say about a week.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Held {
    Ready,
    Waiting { earlier: bool },
    Failed { earlier: bool },
}

pub fn build(ctx: &Rc<Ctx>) -> adw::NavigationPage {
    let title = adw::WindowTitle::new(&t::nav_calendar(), "");
    let header = adw::HeaderBar::new();
    header.pack_start(&widgets::sidebar_button(&ctx.split));
    header.pack_start(&crate::window::tv_session_button(ctx));
    header.set_title_widget(Some(&title));
    let previous = gtk::Button::from_icon_name("go-previous-symbolic");
    previous.set_tooltip_text(Some(&t::calendar_previous_week()));
    let next = gtk::Button::from_icon_name("go-next-symbolic");
    next.set_tooltip_text(Some(&t::calendar_next_week()));
    // Today stays between the arrows, so repeated clicks never move them, and is off on this
    // week, which also says where you are.
    let today = gtk::Button::with_label(&t::calendar_today());
    today.set_sensitive(false);
    today.add_css_class("calendar-today");
    let steps = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    steps.add_css_class("linked");
    steps.append(&previous);
    steps.append(&today);
    steps.append(&next);
    header.pack_end(&steps);

    // All airing leads and is the default; the pick is remembered as a setting.
    let mine = ctx
        .hub
        .known
        .borrow()
        .setting(SettingKey::CalendarMine)
        .is_some_and(|row| row.value == "on");
    let toggles = adw::ToggleGroup::new();
    toggles.add(adw::Toggle::builder().name("all").label(t::calendar_filter_all()).build());
    toggles.add(adw::Toggle::builder().name("mine").label(t::calendar_filter_mine()).build());
    toggles.set_active_name(Some(if mine { "mine" } else { "all" }));
    toggles.add_css_class("round");
    toggles.set_halign(gtk::Align::Center);
    toggles.set_margin_top(6);
    toggles.set_margin_bottom(6);

    let grid = || {
        let grid = gtk::Grid::new();
        grid.set_column_homogeneous(true);
        grid.set_column_spacing(4);
        grid.set_margin_start(12);
        grid.set_margin_end(12);
        grid.set_margin_bottom(12);
        grid
    };
    let grids = [grid(), grid()];
    // Weeks slide over the standard token's 360 ms; a week changing in place crossfades.
    let week_stack = gtk::Stack::new();
    week_stack.set_transition_duration(360);
    week_stack.set_hhomogeneous(true);
    week_stack.add_named(&grids[0], Some("a"));
    week_stack.add_named(&grids[1], Some("b"));
    let agenda = gtk::Box::new(gtk::Orientation::Vertical, 18);
    agenda.set_margin_start(18);
    agenda.set_margin_end(18);
    agenda.set_margin_top(6);
    agenda.set_margin_bottom(24);
    let scroll_week = gtk::ScrolledWindow::builder()
        .hscrollbar_policy(gtk::PolicyType::Never)
        .vexpand(true)
        .child(&week_stack)
        .build();
    let scroll_agenda = gtk::ScrolledWindow::builder()
        .hscrollbar_policy(gtk::PolicyType::Never)
        .vexpand(true)
        .child(&adw::Clamp::builder().maximum_size(760).child(&agenda).build())
        .build();
    let empty = states::plain(&t::calendar_empty_title(), &t::calendar_empty_mine());
    let views = gtk::Stack::new();
    views.add_named(&scroll_week, Some("week"));
    views.add_named(&scroll_agenda, Some("agenda"));
    let body = States::new(&ctx.hub, &views, Shape::Week, &empty, &t::calendar_load_failed());

    let column = gtk::Box::new(gtk::Orientation::Vertical, 0);
    column.append(&toggles);
    column.append(&body.widget);
    let navigation = widgets::content_page(&t::nav_calendar(), &header, &column);

    let page = Rc::new(Page {
        ctx: Rc::clone(ctx),
        me: RefCell::default(),
        rows: RefCell::default(),
        anchor: Cell::new(None),
        floor: Cell::new(None),
        ceiling: Cell::new(None),
        pages: Cell::new(0),
        more_past: Cell::new(false),
        asking_past: Cell::new(false),
        failed_past: Cell::new(false),
        ahead_pages: Cell::new(0),
        more_ahead: Cell::new(false),
        asking_ahead: Cell::new(false),
        failed_ahead: Cell::new(false),
        week: Cell::new(0),
        mine: Cell::new(mine),
        syncing: Cell::new(false),
        title,
        today: today.clone(),
        previous: previous.clone(),
        next: next.clone(),
        toggles: toggles.clone(),
        body,
        views,
        week_stack,
        grids,
        grid_on: Cell::new(0),
        shown_week: Cell::new(None),
        look: Cell::new(Look::Cards),
        waiting: Cell::new(None),
        wait_timer: RefCell::default(),
        since: Cell::new(None),
        agenda,
        empty,
        loaded: Cell::new(false),
        answered: Cell::new(false),
    });
    page.me.replace(Rc::downgrade(&page));

    let weak = Rc::downgrade(&page);
    ctx.hub.listen(move |event| {
        if let Some(page) = weak.upgrade() {
            page.heard(event);
        }
    });
    page.body.on_retry({
        let weak = Rc::downgrade(&page);
        move || {
            if let Some(page) = weak.upgrade() {
                page.load();
            }
        }
    });
    let step = |delta: i32| {
        let weak = Rc::downgrade(&page);
        move |_: &gtk::Button| {
            if let Some(page) = weak.upgrade() {
                page.step(delta);
            }
        }
    };
    previous.connect_clicked(step(-1));
    next.connect_clicked(step(1));
    today.connect_clicked(step(0));
    toggles.connect_active_name_notify({
        let weak = Rc::downgrade(&page);
        move |toggles| {
            if let Some(page) = weak.upgrade()
                && !page.syncing.get()
            {
                let mine = toggles.active_name().as_deref() == Some("mine");
                page.mine.set(mine);
                page.ctx.hub.send(Command::SetSetting {
                    key: SettingKey::CalendarMine,
                    value: SettingValue::Bool(mine),
                });
                page.show();
            }
        }
    });
    ctx.on_size({
        let weak = Rc::downgrade(&page);
        move |_| {
            if let Some(page) = weak.upgrade() {
                page.show();
            }
        }
    });
    // Ctrl+T, Ctrl+Left and Ctrl+Right, while Calendar is on screen. Alt+Left is Back.
    let keys = gtk::ShortcutController::new();
    for (accel, delta) in [("<Control>t", 0), ("<Control>Left", -1), ("<Control>Right", 1)] {
        keys.add_shortcut(gtk::Shortcut::new(
            gtk::ShortcutTrigger::parse_string(accel),
            Some(gtk::CallbackAction::new({
                let weak = Rc::downgrade(&page);
                let navigation = navigation.downgrade();
                move |_, _| {
                    let (Some(page), Some(navigation)) = (weak.upgrade(), navigation.upgrade())
                    else {
                        return glib::Propagation::Proceed;
                    };
                    if !navigation.is_mapped() {
                        return glib::Propagation::Proceed;
                    }
                    page.step(delta);
                    glib::Propagation::Stop
                }
            })),
        ));
    }
    ctx.window.add_controller(keys);
    // The line that marks now moves with the clock, and today moves at midnight.
    glib::timeout_add_seconds_local(60, {
        let weak = Rc::downgrade(&page);
        move || match weak.upgrade() {
            Some(page) => {
                if page.week.get() == 0 && page.views.is_mapped() {
                    page.show();
                }
                glib::ControlFlow::Continue
            }
            None => glib::ControlFlow::Break,
        }
    });
    // Loaded the first time it is shown, and again each time after a while away.
    navigation.connect_map({
        let weak = Rc::downgrade(&page);
        move |_| {
            if let Some(page) = weak.upgrade()
                && !page.loaded.replace(true)
            {
                page.load();
            }
        }
    });
    page.show();
    widgets::keep(&navigation, page);
    navigation
}

impl Page {
    fn load(&self) {
        self.body.loading(Some(t::activity_calendar()));
        self.ctx.hub.send(Command::LoadCalendar);
    }

    /// To the week `delta` from this one, or back to this one with 0.
    fn step(&self, delta: i32) {
        let week = if delta == 0 { 0 } else { self.week.get() + delta };
        let days = week_days(week);
        // A week that lies wholly outside what the calendar reaches is not stepped to.
        if self.reach_of_week(&days).iter().all(|r| *r != Reach::In) {
            return;
        }
        self.week.set(week);
        self.show();
    }

    fn heard(&self, event: &Event) {
        match event {
            Event::CalendarWindow {
                anchor,
                pages,
                more_past,
                floor,
                ceiling,
                ahead_pages,
                more_ahead,
            } => {
                self.anchor.set(Some(*anchor));
                self.floor.set(Some(*floor));
                self.ceiling.set(Some(*ceiling));
                self.pages.set(*pages);
                self.more_past.set(*more_past);
                self.ahead_pages.set(*ahead_pages);
                self.more_ahead.set(*more_ahead);
                for flag in [
                    &self.asking_past,
                    &self.failed_past,
                    &self.asking_ahead,
                    &self.failed_ahead,
                ] {
                    flag.set(false);
                }
            }
            Event::Content { view: ContentFor::Calendar, content } => match content {
                ContentList::Loading => self.body.loading(Some(t::activity_calendar())),
                ContentList::Failed(why) => {
                    tracing::warn!(%why, "the calendar did not load");
                    self.answered.set(false);
                    self.body.failed(why);
                }
                _ => {
                    *self.rows.borrow_mut() = content.entries().to_vec();
                    self.answered.set(true);
                    self.show();
                }
            },
            Event::CalendarBackfill { page, entries, more_past } => {
                self.asking_past.set(false);
                if let Some(page) = page {
                    self.failed_past.set(false);
                    self.pages.set(*page);
                    self.more_past.set(*more_past);
                    let mut rows = entries.clone();
                    rows.extend(self.rows.borrow_mut().drain(..));
                    *self.rows.borrow_mut() = rows;
                } else {
                    self.failed_past.set(true);
                }
                self.show();
            }
            Event::CalendarAhead { page, entries, more_ahead } => {
                self.asking_ahead.set(false);
                if let Some(page) = page {
                    self.failed_ahead.set(false);
                    self.ahead_pages.set(*page);
                    self.more_ahead.set(*more_ahead);
                    self.rows.borrow_mut().extend(entries.iter().cloned());
                } else {
                    self.failed_ahead.set(true);
                }
                self.show();
            }
            // The pick may have been changed elsewhere, in Settings.
            Event::Settings(_) => self.follow_setting(),
            // Progress moved: a broadcast may be watched now.
            Event::Progress { .. } if !self.rows.borrow().is_empty() => self.show(),
            // Titles are named another way now: the week again, once it has been shown.
            Event::TitlesChanged(_) if self.answered.get() => self.load(),
            _ => {}
        }
    }

    /// The toggle follows the Calendar setting when something else changed it.
    fn follow_setting(&self) {
        let mine = self
            .ctx
            .hub
            .known
            .borrow()
            .setting(SettingKey::CalendarMine)
            .is_some_and(|row| row.value == "on");
        if mine != self.mine.get() {
            self.mine.set(mine);
            self.syncing.set(true);
            self.toggles.set_active_name(Some(if mine { "mine" } else { "all" }));
            self.syncing.set(false);
            self.show();
        }
    }

    /// How each of the week's days stands against the days the calendar reaches: the floor is
    /// three months back and the ceiling four weeks ahead, both from the engine.
    fn reach_of_week(&self, days: &[glib::DateTime]) -> Vec<Reach> {
        days.iter()
            .map(|day| {
                let start = day.to_unix();
                let end = day.add_days(1).map_or(start + 86_400, |d| d.to_unix());
                match (self.floor.get(), self.ceiling.get()) {
                    (Some(floor), _) if end <= floor => Reach::Before,
                    (_, Some(ceiling)) if start > ceiling => Reach::After,
                    _ => Reach::In,
                }
            })
            .collect()
    }

    /// The span of time the loaded broadcasts account for: all the way to a side that has
    /// nothing more, else as far as the last broadcast loaded on it.
    fn held(&self) -> (i64, i64) {
        let anchor = self.anchor.get().unwrap_or_else(now);
        let rows = self.rows.borrow();
        let airs = rows.iter().filter_map(|r| r.airing_in).map(|s| anchor + s);
        let from =
            if self.more_past.get() { airs.clone().min().unwrap_or(anchor) } else { i64::MIN };
        let to = if self.more_ahead.get() { airs.max().unwrap_or(anchor) } else { i64::MAX };
        (from, to)
    }

    /// Whether the loaded broadcasts reach `start` to `end`, asking for what is missing: the
    /// calendar pages in as a week is stepped to.
    fn hold(&self, start: i64, end: i64) -> Held {
        let (from, to) = self.held();
        let mut held = Held::Ready;
        if start < from {
            held = if self.failed_past.get() {
                Held::Failed { earlier: true }
            } else {
                self.ask_past(start);
                Held::Waiting { earlier: true }
            };
        }
        if end > to && !matches!(held, Held::Waiting { .. }) {
            held = if self.failed_ahead.get() {
                Held::Failed { earlier: false }
            } else {
                self.ask_ahead(end);
                Held::Waiting { earlier: false }
            };
        }
        held
    }

    fn ask_past(&self, through: i64) {
        let Some(anchor) = self.anchor.get() else { return };
        if !self.more_past.get() || self.asking_past.replace(true) {
            return;
        }
        self.ctx.hub.send(Command::LoadCalendarPast {
            anchor,
            page: self.pages.get() + 1,
            through: Some(through),
        });
    }

    fn ask_ahead(&self, through: i64) {
        let Some(anchor) = self.anchor.get() else { return };
        if !self.more_ahead.get() || self.asking_ahead.replace(true) {
            return;
        }
        self.ctx.hub.send(Command::LoadCalendarAhead {
            anchor,
            page: self.ahead_pages.get() + 1,
            through: Some(through),
        });
    }

    /// A week away from what is loaded is asked for before it is stepped to, so ordinary
    /// stepping never waits.
    fn prefetch(&self, start: i64, end: i64) {
        let (from, to) = self.held();
        let floor = self.floor.get().unwrap_or(i64::MIN);
        let ceiling = self.ceiling.get().unwrap_or(i64::MAX);
        let before = (start - 7 * 86_400).max(floor);
        let after = (end + 7 * 86_400).min(ceiling);
        if before < from && !self.failed_past.get() {
            self.ask_past(before);
        }
        if after > to && !self.failed_ahead.get() {
            self.ask_ahead(after);
        }
    }

    /// The broadcasts, oldest first, each title as it now stands.
    fn broadcasts(&self, mine: bool) -> Vec<Broadcast> {
        let anchor = self.anchor.get().unwrap_or_else(now);
        let known = self.ctx.hub.known.borrow();
        let mut broadcasts: Vec<Broadcast> = self
            .rows
            .borrow()
            .iter()
            .filter_map(|row| {
                let (airing_in, episode) = (row.airing_in?, row.airing_episode?);
                let mut entry = row.clone();
                if let Some(now) = known.entries.get(&row.id.get()) {
                    entry.progress.clone_from(&now.progress);
                    entry.list_status = now.list_status.or(entry.list_status);
                }
                if mine && !is_mine(&entry) {
                    return None;
                }
                Some(Broadcast {
                    entry,
                    episode,
                    airs: anchor + airing_in,
                    was: row.delayed_from,
                })
            })
            .collect();
        broadcasts.sort_by_key(|b| b.airs);
        broadcasts
    }

    fn show(&self) {
        let days = week_days(self.week.get());
        let reach = self.reach_of_week(&days);
        let (title, subtitle) = week_name(&days, month_first());
        self.title.set_title(&title);
        if self.title.subtitle() != subtitle {
            self.title.set_subtitle(&subtitle);
            // The range changes with one standard crossfade.
            if self.title.is_mapped() {
                motion::crossfade_in(&self.title);
            }
        }
        self.today.set_sensitive(self.week.get() != 0);
        // The arrows stop where the calendar does.
        let beyond = |offset: i32| {
            self.reach_of_week(&week_days(self.week.get() + offset))
                .iter()
                .all(|r| *r != Reach::In)
        };
        self.previous.set_sensitive(!beyond(-1));
        self.next.set_sensitive(!beyond(1));
        self.body.agenda(self.ctx.size() != Size::Wide);
        if !self.loaded.get()
            || self.rows.borrow().is_empty() && self.anchor.get().is_none()
            || !self.answered.get()
        {
            return;
        }
        // The days of the week that the calendar reaches.
        let first = reach.iter().position(|r| *r == Reach::In).unwrap_or(0);
        let last = reach.iter().rposition(|r| *r == Reach::In).unwrap_or(6);
        let start = days[first].to_unix();
        let end = days[last].add_days(1).map_or(start + 7 * 86_400, |d| d.to_unix());
        let held = self.hold(start, end);
        if let Held::Waiting { earlier } = held {
            if self.ctx.size() == Size::Wide {
                self.wait_week(&days, &reach);
                return;
            }
            self.body.loading(Some(if earlier {
                t::calendar_loading_earlier()
            } else {
                t::calendar_loading_later()
            }));
            return;
        }
        if self.settle() {
            return;
        }
        let ends = if held == Held::Ready { self.schedule_end(&days, &reach) } else { None };
        let broadcasts: Vec<Broadcast> = self
            .broadcasts(self.mine.get())
            .into_iter()
            .filter(|b| b.airs >= start && b.airs < end)
            .collect();
        // A line in the week's place: the load failed, or nothing on the list airs.
        let note = match held {
            Held::Failed { earlier } => Some(self.note(
                &if earlier {
                    t::calendar_load_earlier_failed()
                } else {
                    t::calendar_load_later_failed()
                },
                &t::action_try_again(),
                true,
            )),
            _ if broadcasts.is_empty() && self.mine.get() => {
                Some(self.note(&t::calendar_empty_mine(), &t::calendar_show_all(), false))
            }
            _ => None,
        };
        if broadcasts.is_empty() && note.is_none() && ends.is_none() {
            self.empty.set_description(Some(&t::calendar_empty_all()));
            self.body.empty();
            return;
        }
        if self.ctx.size() == Size::Wide {
            self.show_week(&days, &reach, &broadcasts, note.as_ref(), ends, Look::Cards);
            self.views.set_visible_child_name("week");
        } else {
            self.show_agenda(&days, &reach, &broadcasts, note.as_ref(), ends);
            self.views.set_visible_child_name("agenda");
        }
        self.body.content();
        self.prefetch(start, end);
    }

    /// A line in a week's place with the one button that answers it: Try again for a failed
    /// page (vermilion on the glyph only), Show all airing for an empty list.
    fn note(&self, line: &str, button: &str, failed: bool) -> gtk::Widget {
        let column = gtk::Box::new(gtk::Orientation::Vertical, 8);
        column.set_halign(gtk::Align::Center);
        column.set_margin_top(24);
        column.set_margin_bottom(24);
        let words = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        words.set_halign(gtk::Align::Center);
        if failed {
            let glyph = gtk::Image::from_icon_name("dialog-warning-symbolic");
            glyph.add_css_class("error");
            words.append(&glyph);
        }
        words.append(&widgets::haze(line, ""));
        column.append(&words);
        let button = gtk::Button::with_label(button);
        button.set_halign(gtk::Align::Center);
        button.add_css_class("pill");
        let weak = self.me.borrow().clone();
        button.connect_clicked(move |_| {
            let Some(page) = weak.upgrade() else { return };
            if failed {
                page.failed_past.set(false);
                page.failed_ahead.set(false);
                page.show();
            } else {
                page.toggles.set_active_name(Some("all"));
            }
        });
        column.append(&button);
        column.upcast()
    }

    fn show_week(
        &self,
        days: &[glib::DateTime],
        reach: &[Reach],
        broadcasts: &[Broadcast],
        note: Option<&gtk::Widget>,
        ends: Option<usize>,
        look: Look,
    ) {
        // The week is built on the grid that is not showing, then arrives by the right motion.
        let on = 1 - self.grid_on.get();
        let grid = &self.grids[on];
        while let Some(child) = grid.first_child() {
            grid.remove(&child);
        }
        let now = now();
        let today = glib::DateTime::now_local().ok();
        for (col, day) in days.iter().enumerate() {
            let outside = reach[col] != Reach::In;
            let col = col as i32;
            let is_today = today.as_ref().is_some_and(|t| same_day(t, day));
            let head = gtk::Box::new(gtk::Orientation::Vertical, 0);
            head.add_css_class("calendar-head");
            // Today's column is raised from its heading to its foot.
            let body = gtk::Box::new(gtk::Orientation::Vertical, 8);
            body.add_css_class("calendar-body");
            if is_today {
                head.add_css_class("today");
                body.add_css_class("today");
            }
            // "Today" takes the weekday's place, in silk; the other days name theirs in haze.
            let name = gtk::Label::new(Some(&if is_today {
                t::calendar_today()
            } else {
                day.format("%a").map(|s| s.to_string()).unwrap_or_default()
            }));
            name.add_css_class("caption-heading");
            if !is_today || outside {
                name.add_css_class("haze");
            }
            name.set_xalign(0.0);
            let date = gtk::Label::new(Some(&day.day_of_month().to_string()));
            date.add_css_class("title-3");
            if outside {
                date.add_css_class("haze");
            }
            date.set_xalign(0.0);
            head.append(&name);
            head.append(&date);
            grid.attach(&head, col, 0, 1, 1);
            if outside || note.is_some() || ends.is_some_and(|e| col as usize >= e) {
                grid.attach(&body, col, 1, 1, 1);
                continue;
            }
            match look {
                Look::Empty => {
                    grid.attach(&body, col, 1, 1, 1);
                    continue;
                }
                Look::Placeholders => {
                    states::column_placeholder(&body);
                    grid.attach(&body, col, 1, 1, 1);
                    continue;
                }
                Look::Cards => {}
            }
            let on_day: Vec<&Broadcast> =
                broadcasts.iter().filter(|b| same_day(&local(b.airs), day)).collect();
            if on_day.is_empty() {
                body.append(&widgets::haze(&t::calendar_linux_day_empty(), "caption"));
            }
            // The line that marks now sits between the last row out and the next, today only.
            let line = (is_today && !on_day.is_empty())
                .then(|| on_day.iter().take_while(|b| b.airs <= now).count());
            for (n, broadcast) in on_day.iter().enumerate() {
                if line == Some(n) {
                    body.append(&now_line(now));
                }
                body.append(&self.cell(broadcast, now));
            }
            if line == Some(on_day.len()) {
                body.append(&now_line(now));
            }
            grid.attach(&body, col, 1, 1, 1);
        }
        // One line spans the days outside the calendar, and a note spans the days it reaches.
        let before = reach.iter().filter(|r| **r == Reach::Before).count() as i32;
        let after = reach.iter().filter(|r| **r == Reach::After).count() as i32;
        let inside = 7 - before - after;
        if before > 0 {
            grid.attach(&edge_line(&t::calendar_floor()), 0, 1, before, 1);
        }
        if after > 0 {
            grid.attach(&edge_line(&t::calendar_ceiling()), 7 - after, 1, after, 1);
        }
        if let Some(note) = note {
            grid.attach(note, before, 1, inside.max(1), 1);
        } else if let Some(e) = ends {
            let span = (7 - after - e as i32).max(1);
            grid.attach(&edge_line(&t::calendar_end()), e as i32, 1, span, 1);
        }
        self.arrive(on, self.week.get(), look);
    }

    /// Bring the built grid in: a step slides one week towards the side it went, a week that
    /// changes what it holds crossfades, and a refresh of the same look swaps in place.
    fn arrive(&self, on: usize, week: i32, look: Look) {
        let transition = match self.shown_week.get() {
            Some(shown) if shown != week => {
                if week > shown {
                    gtk::StackTransitionType::SlideLeft
                } else {
                    gtk::StackTransitionType::SlideRight
                }
            }
            Some(_) if self.look.get() != look => gtk::StackTransitionType::Crossfade,
            _ => gtk::StackTransitionType::None,
        };
        self.week_stack.set_transition_type(transition);
        self.week_stack.set_visible_child_name(if on == 0 { "a" } else { "b" });
        self.grid_on.set(on);
        self.shown_week.set(Some(week));
        self.look.set(look);
    }

    /// The week's broadcasts are on their way: its columns arrive empty, placeholders follow
    /// after 600 ms of quiet and stand at least 0.3 s.
    fn wait_week(&self, days: &[glib::DateTime], reach: &[Reach]) {
        let week = self.week.get();
        if self.waiting.get() == Some(week) {
            return;
        }
        self.cancel_wait();
        self.waiting.set(Some(week));
        self.show_week(days, reach, &[], None, None, Look::Empty);
        self.views.set_visible_child_name("week");
        self.body.content();
        let weak = self.me.borrow().clone();
        let id = glib::timeout_add_local_once(states::SILENT, move || {
            let Some(page) = weak.upgrade() else { return };
            // This timer has fired: forgotten, so a cancel does not remove it again.
            page.wait_timer.borrow_mut().take();
            if page.waiting.get() == Some(week) && page.week.get() == week {
                let days = week_days(week);
                let reach = page.reach_of_week(&days);
                page.show_week(&days, &reach, &[], None, None, Look::Placeholders);
                page.since.set(Some(Instant::now()));
            }
        });
        self.wait_timer.replace(Some(id));
    }

    fn cancel_wait(&self) {
        if let Some(id) = self.wait_timer.borrow_mut().take() {
            id.remove();
        }
    }

    /// The broadcasts arrived. True when placeholders are up and have not stood their least
    /// time yet: the cards come once they have, in one crossfade.
    fn settle(&self) -> bool {
        if self.waiting.get().is_none() {
            return false;
        }
        let left =
            self.since.get().and_then(|since| states::AT_LEAST.checked_sub(since.elapsed()));
        let Some(left) = left else {
            self.cancel_wait();
            self.waiting.set(None);
            self.since.set(None);
            return false;
        };
        if self.wait_timer.borrow().is_none() {
            let weak = self.me.borrow().clone();
            let id = glib::timeout_add_local_once(left, move || {
                let Some(page) = weak.upgrade() else { return };
                page.wait_timer.borrow_mut().take();
                page.waiting.set(None);
                page.since.set(None);
                page.show();
            });
            self.wait_timer.replace(Some(id));
        }
        true
    }

    /// Where the schedule runs out before the calendar does: the first column of this week
    /// after the last broadcast there is, once the engine has no more pages ahead.
    fn schedule_end(&self, days: &[glib::DateTime], reach: &[Reach]) -> Option<usize> {
        if self.more_ahead.get() || self.anchor.get().is_none() {
            return None;
        }
        let last = self.broadcasts(false).last()?.airs;
        (0..7).find(|&n| reach[n] == Reach::In && days[n].to_unix() > last)
    }

    /// A broadcast in a day's column: its art, the time, the title and the episode line; one
    /// already watched is a quiet line.
    fn cell(&self, broadcast: &Broadcast, now: i64) -> gtk::Widget {
        let entry = &broadcast.entry;
        let cell = gtk::Box::new(gtk::Orientation::Vertical, 4);
        if broadcast.watched() {
            // The same line as the others, in the same order, then the title.
            let line =
                widgets::haze(&format!("{} · {}", broadcast.line(), entry.title), "caption");
            line.set_wrap(false);
            line.set_ellipsize(gtk::pango::EllipsizeMode::End);
            cell.append(&line);
        } else {
            cell.append(&widgets::banner(&self.ctx.hub, entry, 0, "small"));
            let title = widgets::show_title(&entry.title, "row");
            title.set_lines(2);
            title.set_ellipsize(gtk::pango::EllipsizeMode::End);
            cell.append(&title);
            cell.append(&episode_line(broadcast, now, false));
        }
        opens(&cell, broadcast)
    }

    fn show_agenda(
        &self,
        days: &[glib::DateTime],
        reach: &[Reach],
        broadcasts: &[Broadcast],
        note: Option<&gtk::Widget>,
        ends: Option<usize>,
    ) {
        clear(&self.agenda);
        let now = now();
        let today = glib::DateTime::now_local().ok();
        let narrow = self.ctx.size() == Size::Narrow;
        // The line at the top or foot where the calendar stops.
        if reach.contains(&Reach::Before) {
            self.agenda.append(&edge_line(&t::calendar_floor()));
        }
        if let Some(note) = note {
            self.agenda.append(note);
        }
        // Today first, then the days after it, then the days of the week already gone.
        let at =
            today.as_ref().and_then(|t| days.iter().position(|d| same_day(t, d))).unwrap_or(0);
        for (n, day) in days.iter().enumerate().skip(at).chain(days.iter().enumerate().take(at))
        {
            if note.is_some() || reach[n] != Reach::In {
                continue;
            }
            let on_day: Vec<&Broadcast> =
                broadcasts.iter().filter(|b| same_day(&local(b.airs), day)).collect();
            let is_today = today.as_ref().is_some_and(|t| same_day(t, day));
            // Today always has its heading, so the agenda says where today is.
            if on_day.is_empty() && !is_today {
                continue;
            }
            let name = day_heading(day, month_first());
            let heading = widgets::heading(&if is_today {
                t::calendar_today_heading(&name)
            } else {
                name
            });
            let section = gtk::Box::new(gtk::Orientation::Vertical, 8);
            section.append(&heading);
            if on_day.is_empty() {
                section.append(&widgets::haze(&t::calendar_empty_today(), ""));
            } else {
                let list = gtk::ListBox::new();
                list.add_css_class("boxed-list");
                list.set_selection_mode(gtk::SelectionMode::None);
                let line =
                    is_today.then(|| on_day.iter().take_while(|b| b.airs <= now).count());
                for (n, broadcast) in on_day.iter().enumerate() {
                    if line == Some(n) {
                        list.append(&now_row(now));
                    }
                    list.append(&self.agenda_row(broadcast, now, narrow));
                }
                if line == Some(on_day.len()) {
                    list.append(&now_row(now));
                }
                section.append(&list);
            }
            self.agenda.append(&section);
        }
        if ends.is_some() && note.is_none() {
            self.agenda.append(&edge_line(&t::calendar_end()));
        }
        if reach.contains(&Reach::After) {
            self.agenda.append(&edge_line(&t::calendar_ceiling()));
        }
    }

    fn agenda_row(&self, broadcast: &Broadcast, now: i64, narrow: bool) -> gtk::ListBoxRow {
        let entry = &broadcast.entry;
        let (width, height) = if narrow { (80, 45) } else { (96, 54) };
        let line = gtk::Box::new(gtk::Orientation::Horizontal, 12);
        line.set_margin_top(8);
        line.set_margin_bottom(8);
        line.set_margin_start(8);
        line.set_margin_end(12);
        let url = entry.banner_url.as_deref().or(entry.cover_url.as_deref());
        let (art, _) = widgets::art(&self.ctx.hub, url, entry.cover_color.as_deref(), "still");
        let art = Fit::new(&art, f64::from(width) / f64::from(height), width, true, 0);
        art.set_valign(gtk::Align::Center);
        line.append(&art);
        let text = gtk::Box::new(gtk::Orientation::Vertical, 2);
        text.set_valign(gtk::Align::Center);
        text.set_hexpand(true);
        let title = widgets::show_title(&entry.title, "row");
        title.set_wrap(false);
        title.set_ellipsize(gtk::pango::EllipsizeMode::End);
        text.append(&title);
        text.append(&episode_line(broadcast, now, true));
        line.append(&text);
        if broadcast.watched() {
            line.add_css_class("dim-label");
            let done = gtk::Image::from_icon_name("object-select-symbolic");
            done.set_tooltip_text(Some(&t::episode_watched()));
            line.append(&done);
        }
        let row = gtk::ListBoxRow::builder().child(&line).activatable(true).build();
        row.update_property(&[gtk::accessible::Property::Label(&format!(
            "{}, {}",
            entry.title,
            broadcast.spoken()
        ))]);
        row.set_action_name(Some("win.open-title"));
        row.set_action_target_value(Some(&entry.id.get().to_variant()));
        widgets::title_menu(&row, entry.id.get(), entry.list_status.is_none());
        row
    }
}

/// The episode line, with the water dot before it when it is out and not watched: one line, cut
/// short rather than wrapped. Where it moved from follows it on the agenda's wide rows, and is
/// the tooltip in the week's narrow columns.
fn episode_line(broadcast: &Broadcast, now: i64, roomy: bool) -> gtk::Box {
    let line = gtk::Box::new(gtk::Orientation::Horizontal, 6);
    if broadcast.is_new(now) {
        let dot = gtk::Box::new(gtk::Orientation::Horizontal, 0);
        dot.add_css_class("state-dot");
        dot.set_valign(gtk::Align::Center);
        dot.set_tooltip_text(Some(&t::calendar_out_unwatched()));
        line.append(&dot);
    }
    let words = match (broadcast.was, roomy) {
        (Some(was), true) => {
            format!("{} · {}", broadcast.line(), t::calendar_moved_from(&time(was)))
        }
        _ => broadcast.line(),
    };
    let text = widgets::haze(&words, "caption");
    text.add_css_class("numeric");
    // One line either way. A row keeps its label wrapping, cut at one line, so the row is
    // measured height for width with its art.
    if roomy {
        text.set_lines(1);
    } else {
        text.set_wrap(false);
    }
    text.set_ellipsize(gtk::pango::EllipsizeMode::End);
    if let Some(moved) = broadcast.moved().filter(|_| !roomy) {
        text.set_tooltip_text(Some(&moved));
    }
    line.append(&text);
    line
}

/// A cell as a flat button that opens its title, read as the title and its episode line, with
/// the title's menu on right-click and on the keyboard.
fn opens(cell: &gtk::Box, broadcast: &Broadcast) -> gtk::Widget {
    let entry = &broadcast.entry;
    let id = entry.id.get();
    let button = gtk::Button::builder().child(cell).build();
    button.add_css_class("flat");
    button.add_css_class("broadcast");
    button.set_action_name(Some("win.open-title"));
    button.set_action_target_value(Some(&id.to_variant()));
    let tip = match broadcast.moved() {
        Some(moved) => format!("{}\n{moved}", entry.title),
        None => entry.title.clone(),
    };
    button.set_tooltip_text(Some(&tip));
    button.update_property(&[gtk::accessible::Property::Label(&format!(
        "{}, {}",
        entry.title,
        broadcast.spoken()
    ))]);
    widgets::title_menu(&button, id, entry.list_status.is_none());
    button.upcast()
}

/// The line that marks now, in silk: the time, then a 1 px rule. It sits between the last
/// broadcast out and the next, and a screen reader says "Now, 16:42".
fn now_line(now: i64) -> gtk::Widget {
    let line = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    line.add_css_class("calendar-now");
    line.set_size_request(-1, 18);
    let at = gtk::Label::new(Some(&time(now)));
    at.add_css_class("numeric");
    at.add_css_class("calendar-now-time");
    let rule = gtk::Separator::new(gtk::Orientation::Horizontal);
    rule.set_hexpand(true);
    rule.set_valign(gtk::Align::Center);
    line.append(&at);
    line.append(&rule);
    line.update_property(&[gtk::accessible::Property::Label(&t::calendar_now_spoken(&time(
        now,
    )))]);
    line.upcast()
}

/// The same line as a row of the agenda's list, which cannot be picked.
fn now_row(now: i64) -> gtk::ListBoxRow {
    let row = gtk::ListBoxRow::builder()
        .child(&now_line(now))
        .activatable(false)
        .selectable(false)
        .build();
    row.add_css_class("calendar-now-row");
    row
}

/// Where the calendar stops: one line in haze over the days it does not reach.
fn edge_line(text: &str) -> gtk::Widget {
    let line = widgets::haze(text, "");
    line.set_wrap(true);
    line.set_halign(gtk::Align::Center);
    line.set_justify(gtk::Justification::Center);
    line.set_margin_top(12);
    line.set_margin_bottom(12);
    line.upcast()
}

fn clear(container: &gtk::Box) {
    while let Some(child) = container.first_child() {
        container.remove(&child);
    }
}

fn now() -> i64 {
    glib::DateTime::now_utc().map_or(0, |t| t.to_unix())
}

fn local(unix: i64) -> glib::DateTime {
    glib::DateTime::from_unix_local(unix)
        .unwrap_or_else(|_| glib::DateTime::from_unix_utc(0).expect("the epoch"))
}

/// "16:30", or "4:30 PM" where the clock is set to 12 hours.
fn time(unix: i64) -> String {
    thread_local! {
        static TWELVE: bool = twelve_hour();
    }
    let format = if TWELVE.with(|t| *t) { "%-l:%M %p" } else { "%H:%M" };
    local(unix).format(format).map(|s| s.trim().to_string()).unwrap_or_default()
}

/// Whether times read on a 12-hour clock: GNOME's clock setting, through the settings portal
/// (which a sandbox allows) or else straight from GSettings, otherwise the locale's own way of
/// writing a time.
pub fn twelve_hour() -> bool {
    if let Some(format) = portal_clock_format() {
        return format == "12h";
    }
    let sandboxed = std::path::Path::new("/.flatpak-info").exists();
    if !sandboxed
        && let Some(source) = gio::SettingsSchemaSource::default()
        && source
            .lookup("org.gnome.desktop.interface", true)
            .is_some_and(|schema| schema.has_key("clock-format"))
    {
        return gio::Settings::new("org.gnome.desktop.interface").string("clock-format")
            == "12h";
    }
    locale_twelve_hour()
}

/// GNOME's clock-format, "12h" or "24h", as the desktop's settings portal reads it out.
fn portal_clock_format() -> Option<String> {
    let bus = gio::bus_get_sync(gio::BusType::Session, gio::Cancellable::NONE).ok()?;
    let ask = |method: &str| {
        bus.call_sync(
            Some("org.freedesktop.portal.Desktop"),
            "/org/freedesktop/portal/desktop",
            "org.freedesktop.portal.Settings",
            method,
            Some(&("org.gnome.desktop.interface", "clock-format").to_variant()),
            None,
            gio::DBusCallFlags::NONE,
            1000,
            gio::Cancellable::NONE,
        )
        .ok()
    };
    // ReadOne answers (v); the older Read answers the value wrapped once more.
    let answer = ask("ReadOne").or_else(|| ask("Read"))?; // i18n-skip
    let mut value = answer.try_child_value(0)?;
    while let Some(inner) = value.as_variant() {
        value = inner;
    }
    value.str().map(str::to_owned)
}

/// Whether the locale writes three in the afternoon without a 15.
fn locale_twelve_hour() -> bool {
    glib::DateTime::new(&glib::TimeZone::utc(), 2026, 1, 1, 15, 0, 0.0)
        .ok()
        .and_then(|t| t.format("%X").ok())
        .is_some_and(|text| !text.contains("15"))
}

fn same_day(a: &glib::DateTime, b: &glib::DateTime) -> bool {
    a.ymd() == b.ymd()
}

/// The days of the week `offset` weeks from this one, Monday first, each at midnight.
fn week_days(offset: i32) -> Vec<glib::DateTime> {
    let today = glib::DateTime::now_local().expect("the local time");
    week_of(&today, offset)
}

fn week_of(day: &glib::DateTime, offset: i32) -> Vec<glib::DateTime> {
    let (year, month, date) = day.ymd();
    let midnight = glib::DateTime::new(&day.timezone(), year, month, date, 0, 0, 0.0)
        .expect("midnight on a real day");
    let monday = midnight.add_days(1 - day.day_of_week() + offset * 7).expect("a day in range");
    (0..7).map(|n| monday.add_days(n).expect("a day in range")).collect()
}

/// Whether the locale writes a date's month before its day: "10/19/26" or "2026-10-19", where
/// another writes "19/10/26".
fn month_first() -> bool {
    thread_local! {
        static MONTH_FIRST: bool = glib::DateTime::new(&glib::TimeZone::utc(), 2026, 11, 22, 12, 0, 0.0)
            .ok()
            .and_then(|t| t.format("%x").ok())
            .is_some_and(|text| month_before_day(&text));
    }
    MONTH_FIRST.with(|m| *m)
}

/// Whether `text`, the locale's way of writing 22 November, puts the 11 before the 22.
fn month_before_day(text: &str) -> bool {
    match (text.find("11"), text.find("22")) {
        (Some(month), Some(day)) => month < day,
        _ => false,
    }
}

/// A day over the agenda's section, in the locale's order: "Monday 19 October", or
/// "Monday, October 19" where the month comes first.
fn day_heading(day: &glib::DateTime, month_first: bool) -> String {
    let format = if month_first { "%A, %B %-d" } else { "%A %-d %B" };
    day.format(format).map(|s| s.to_string()).unwrap_or_default()
}

/// The week as the header's title says it, "19 to 25 October" (or "October 19 to 25" where the
/// month comes first), and its year under it.
fn week_name(days: &[glib::DateTime], month_first: bool) -> (String, String) {
    let (Some(first), Some(last)) = (days.first(), days.last()) else {
        return (String::new(), String::new());
    };
    let month = |d: &glib::DateTime| d.format("%B").map(|s| s.to_string()).unwrap_or_default();
    let date = |d: &glib::DateTime| {
        if month_first {
            format!("{} {}", month(d), d.day_of_month())
        } else {
            format!("{} {}", d.day_of_month(), month(d))
        }
    };
    let title = match (first.month() == last.month(), month_first) {
        (true, true) => format!("{} to {}", date(first), last.day_of_month()),
        (true, false) => format!("{} to {}", first.day_of_month(), date(last)),
        (false, _) => format!("{} to {}", date(first), date(last)),
    };
    let year = if first.year() == last.year() {
        last.year().to_string()
    } else {
        format!("{} to {}", first.year(), last.year())
    };
    (title, year)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn day(y: i32, m: i32, d: i32) -> glib::DateTime {
        glib::DateTime::new(&glib::TimeZone::utc(), y, m, d, 12, 0, 0.0).expect("a day")
    }

    #[test]
    fn a_week_runs_monday_to_sunday() {
        // Friday 23 October 2026.
        let days = week_of(&day(2026, 10, 23), 0);
        assert_eq!(days[0].ymd(), (2026, 10, 19));
        assert_eq!(days[6].ymd(), (2026, 10, 25));
        assert_eq!(week_name(&days, false), ("19 to 25 October".into(), "2026".into()));
        assert_eq!(week_name(&days, true).0, "October 19 to 25");
        // A Monday is its own week's first day.
        assert_eq!(week_of(&day(2026, 10, 19), 0)[0].ymd(), (2026, 10, 19));
        // Across months, and across years.
        let days = week_of(&day(2026, 10, 23), -4);
        assert_eq!(week_name(&days, false).0, "21 to 27 September");
        let days = week_of(&day(2026, 12, 30), 0);
        assert_eq!(
            week_name(&days, false),
            ("28 December to 3 January".into(), "2026 to 2027".into())
        );
        assert_eq!(week_name(&days, true).0, "December 28 to January 3");
    }

    #[test]
    fn a_day_heading_follows_the_locales_order() {
        assert!(month_before_day("11/22/26"));
        assert!(month_before_day("2026-11-22"));
        assert!(!month_before_day("22/11/26"));
        assert!(!month_before_day("22.11.2026"));
        assert!(!month_before_day(""));
        let monday = day(2026, 10, 19);
        assert_eq!(day_heading(&monday, false), "Monday 19 October");
        assert_eq!(day_heading(&monday, true), "Monday, October 19");
    }

    #[test]
    fn a_broadcast_says_its_episode_time_and_delay() {
        let at = day(2026, 10, 23).add_hours(4).expect("16:00").to_unix();
        let mut broadcast = Broadcast {
            entry: Entry::new(anistream_core::ids::AnilistId::new(1), "A"),
            episode: 10,
            airs: at,
            was: None,
        };
        assert!(broadcast.line().starts_with("E10 · "));
        broadcast.was = Some(at - 3600);
        assert!(!broadcast.line().contains("was"), "the line stays one short line");
        assert!(broadcast.moved().is_some_and(|m| m.starts_with("Moved from ")));
        assert!(broadcast.spoken().starts_with("Episode 10, "));
        assert!(broadcast.spoken().contains(", Moved from "));
        assert!(broadcast.is_new(at + 1));
        broadcast.entry.progress =
            Some(anistream_model::progress::TitleProgress { done: 10, next: 11 });
        assert!(!broadcast.is_new(at + 1));
    }

    #[test]
    fn an_estimate_reads_with_a_tilde_and_a_dub_is_marked() {
        use anistream_core::media::Translation;
        let at = day(2026, 10, 23).add_hours(4).expect("16:00").to_unix();
        let mut broadcast = Broadcast {
            entry: Entry::new(anistream_core::ids::AnilistId::new(1), "A"),
            episode: 3,
            airs: at,
            was: None,
        };
        assert!(!broadcast.line().contains('~'));
        assert!(!broadcast.line().contains("dubbed") && !broadcast.line().contains("Sub"));
        broadcast.entry.airing_approximate = true;
        broadcast.entry.airing_translation = Some(Translation::Dub);
        assert!(broadcast.line().starts_with("E3 dubbed · ~"));
        assert!(broadcast.line().contains("dubbed"));
        assert!(!broadcast.spoken().contains('~'), "said as about, not tilde");
        assert!(broadcast.spoken().contains(", about "));
        // A sub is the plain case: no mark.
        broadcast.entry.airing_translation = Some(Translation::Sub);
        assert!(!broadcast.line().contains("Sub"));
    }
}
