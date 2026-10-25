//! Calendar: the week, Monday first, as seven columns on wide windows and as an agenda below
//! that. The week is in the header bar's title with the year under it, previous and next beside
//! it, and My List or All Airing in a toggle group under the header bar.

use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};

use adw::prelude::*;
use anistream_model::{
    catalog::{ContentFor, ContentList, Entry, LibrarySegment},
    command::Command,
    event::Event,
    present,
};
use gtk::{gio, glib};

use crate::{
    fit::Fit,
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

    /// "Ep 10 · 16:30 · Dub", with "(was 23:00)" when it moved. "~16:30" for an estimate, and the
    /// mark only when the engine knows a sub or dub.
    fn line(&self) -> String {
        let mut line = format!("Ep {} · {}", self.episode, self.clock());
        if let Some(mark) = present::calendar_mark(&self.entry) {
            line.push_str(&format!(" · {mark}"));
        }
        if let Some(was) = self.was {
            line.push_str(&format!(" (was {})", time(was)));
        }
        line
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
    rows: RefCell<Vec<Entry>>,
    /// What the rows' times count from, and how to ask for older ones.
    anchor: Cell<Option<i64>>,
    pages: Cell<u32>,
    more_past: Cell<bool>,
    asking_past: Cell<bool>,
    /// Weeks from this one.
    week: Cell<i32>,
    mine: Cell<bool>,
    title: adw::WindowTitle,
    this_week: gtk::Button,
    body: Rc<States>,
    /// The week's columns or the agenda, whichever the width takes.
    views: gtk::Stack,
    week_view: gtk::Box,
    agenda: gtk::Box,
    empty: adw::StatusPage,
    loaded: Cell<bool>,
    /// Whether the week on screen came from an answer, so stepping weeks before one, or after
    /// a failure, leaves the state alone.
    answered: Cell<bool>,
}

pub fn build(ctx: &Rc<Ctx>) -> adw::NavigationPage {
    let title = adw::WindowTitle::new("Calendar", "");
    let header = adw::HeaderBar::new();
    header.pack_start(&widgets::sidebar_button(&ctx.split));
    header.set_title_widget(Some(&title));
    let previous = gtk::Button::from_icon_name("go-previous-symbolic");
    previous.set_tooltip_text(Some("Previous Week"));
    let next = gtk::Button::from_icon_name("go-next-symbolic");
    next.set_tooltip_text(Some("Next Week"));
    let steps = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    steps.add_css_class("linked");
    steps.append(&previous);
    steps.append(&next);
    let this_week = gtk::Button::with_label("This Week");
    this_week.set_visible(false);
    header.pack_end(&steps);
    header.pack_end(&this_week);

    let toggles = adw::ToggleGroup::new();
    toggles.add(adw::Toggle::builder().name("mine").label("My List").build());
    toggles.add(adw::Toggle::builder().name("all").label("All Airing").build());
    toggles.set_active_name(Some("mine"));
    toggles.add_css_class("round");
    toggles.set_halign(gtk::Align::Center);
    toggles.set_margin_top(6);
    toggles.set_margin_bottom(6);

    let week_view = gtk::Box::new(gtk::Orientation::Horizontal, 4);
    week_view.set_homogeneous(true);
    week_view.set_margin_start(12);
    week_view.set_margin_end(12);
    week_view.set_margin_bottom(12);
    let agenda = gtk::Box::new(gtk::Orientation::Vertical, 18);
    agenda.set_margin_start(18);
    agenda.set_margin_end(18);
    agenda.set_margin_top(6);
    agenda.set_margin_bottom(24);
    let scroll_week = gtk::ScrolledWindow::builder()
        .hscrollbar_policy(gtk::PolicyType::Never)
        .vexpand(true)
        .child(&week_view)
        .build();
    let scroll_agenda = gtk::ScrolledWindow::builder()
        .hscrollbar_policy(gtk::PolicyType::Never)
        .vexpand(true)
        .child(&adw::Clamp::builder().maximum_size(760).child(&agenda).build())
        .build();
    let empty = states::plain("Nothing Airing", "Nothing on your list airs this week.");
    let views = gtk::Stack::new();
    views.add_named(&scroll_week, Some("week"));
    views.add_named(&scroll_agenda, Some("agenda"));
    let body = States::new(&ctx.hub, &views, Shape::Week, &empty, "Couldn't Load the Calendar");

    let column = gtk::Box::new(gtk::Orientation::Vertical, 0);
    column.append(&toggles);
    column.append(&body.widget);
    let navigation = widgets::content_page("Calendar", &header, &column);

    let page = Rc::new(Page {
        ctx: Rc::clone(ctx),
        rows: RefCell::default(),
        anchor: Cell::new(None),
        pages: Cell::new(0),
        more_past: Cell::new(false),
        asking_past: Cell::new(false),
        week: Cell::new(0),
        mine: Cell::new(true),
        title,
        this_week: this_week.clone(),
        body,
        views,
        week_view,
        agenda,
        empty,
        loaded: Cell::new(false),
        answered: Cell::new(false),
    });

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
                page.week.set(if delta == 0 { 0 } else { page.week.get() + delta });
                page.show();
                page.backfill();
            }
        }
    };
    previous.connect_clicked(step(-1));
    next.connect_clicked(step(1));
    this_week.connect_clicked(step(0));
    toggles.connect_active_name_notify({
        let weak = Rc::downgrade(&page);
        move |toggles| {
            if let Some(page) = weak.upgrade() {
                page.mine.set(toggles.active_name().as_deref() != Some("all"));
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
        self.body.loading(Command::LoadCalendar.activity());
        self.ctx.hub.send(Command::LoadCalendar);
    }

    fn heard(&self, event: &Event) {
        match event {
            Event::CalendarWindow { anchor, pages, more_past } => {
                self.anchor.set(Some(*anchor));
                self.pages.set(*pages);
                self.more_past.set(*more_past);
            }
            Event::Content { view: ContentFor::Calendar, content } => match content {
                ContentList::Loading => self.body.loading(Command::LoadCalendar.activity()),
                ContentList::Failed(why) => {
                    tracing::warn!(%why, "the calendar did not load");
                    self.answered.set(false);
                    self.body.failed(why);
                }
                _ => {
                    *self.rows.borrow_mut() = content.entries().to_vec();
                    self.answered.set(true);
                    self.show();
                    self.backfill();
                }
            },
            Event::CalendarBackfill { page, entries, more_past } => {
                self.asking_past.set(false);
                if let Some(page) = page {
                    self.pages.set(page + 1);
                    self.more_past.set(*more_past);
                    let mut rows = entries.clone();
                    rows.extend(self.rows.borrow_mut().drain(..));
                    *self.rows.borrow_mut() = rows;
                    self.show();
                    self.backfill();
                }
            }
            // Progress moved: a broadcast may be watched now.
            Event::Progress { .. } if !self.rows.borrow().is_empty() => self.show(),
            _ => {}
        }
    }

    /// An earlier week than is loaded asks for the broadcasts before it.
    fn backfill(&self) {
        let Some(anchor) = self.anchor.get() else { return };
        if self.week.get() >= 0 || !self.more_past.get() || self.asking_past.get() {
            return;
        }
        let first_day = week_days(self.week.get())[0].to_unix();
        let earliest = self.broadcasts(false).first().map(|b| b.airs);
        if earliest.is_some_and(|at| at <= first_day) {
            return;
        }
        self.asking_past.set(true);
        self.ctx.hub.send(Command::LoadCalendarPast { anchor, page: self.pages.get() });
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
        let (title, subtitle) = week_name(&days, month_first());
        self.title.set_title(&title);
        self.title.set_subtitle(&subtitle);
        self.this_week.set_visible(self.week.get() != 0);
        self.body.agenda(self.ctx.size() != Size::Wide);
        if !self.loaded.get()
            || self.rows.borrow().is_empty() && self.anchor.get().is_none()
            || !self.answered.get()
        {
            return;
        }
        let start = days[0].to_unix();
        let end = days[6].add_days(1).map_or(start + 7 * 86_400, |d| d.to_unix());
        let broadcasts: Vec<Broadcast> = self
            .broadcasts(self.mine.get())
            .into_iter()
            .filter(|b| b.airs >= start && b.airs < end)
            .collect();
        if broadcasts.is_empty() {
            self.empty.set_description(Some(if self.mine.get() {
                "Nothing on your list airs this week."
            } else {
                "Nothing airs this week."
            }));
            self.body.empty();
            return;
        }
        if self.ctx.size() == Size::Wide {
            self.show_week(&days, &broadcasts);
            self.views.set_visible_child_name("week");
        } else {
            self.show_agenda(&days, &broadcasts);
            self.views.set_visible_child_name("agenda");
        }
        self.body.content();
    }

    fn show_week(&self, days: &[glib::DateTime], broadcasts: &[Broadcast]) {
        clear(&self.week_view);
        let now = now();
        let today = glib::DateTime::now_local().ok();
        for day in days {
            let column = gtk::Box::new(gtk::Orientation::Vertical, 8);
            column.add_css_class("calendar-day");
            let is_today = today.as_ref().is_some_and(|t| same_day(t, day));
            if is_today {
                column.add_css_class("today");
            }
            let heading = gtk::Box::new(gtk::Orientation::Vertical, 0);
            heading.add_css_class("calendar-heading");
            let name = gtk::Label::new(Some(
                &day.format("%a").map(|s| s.to_string()).unwrap_or_default(),
            ));
            name.add_css_class("caption-heading");
            name.add_css_class("haze");
            name.set_xalign(0.0);
            let date = gtk::Label::new(Some(&day.day_of_month().to_string()));
            date.add_css_class("title-3");
            date.set_xalign(0.0);
            heading.append(&name);
            heading.append(&date);
            column.append(&heading);
            for broadcast in broadcasts.iter().filter(|b| same_day(&local(b.airs), day)) {
                column.append(&self.cell(broadcast, now));
            }
            self.week_view.append(&column);
        }
    }

    /// A broadcast in a day's column: its art, the time, the title and the episode line; one
    /// already watched is a quiet line.
    fn cell(&self, broadcast: &Broadcast, now: i64) -> gtk::Widget {
        let entry = &broadcast.entry;
        let cell = gtk::Box::new(gtk::Orientation::Vertical, 4);
        if broadcast.watched() {
            let line = widgets::haze(
                &format!(
                    "{} · Ep {} · {}",
                    broadcast.clock(),
                    broadcast.episode,
                    entry.title
                ),
                "caption",
            );
            line.set_wrap(false);
            line.set_ellipsize(gtk::pango::EllipsizeMode::End);
            cell.append(&line);
        } else {
            cell.append(&widgets::banner(&self.ctx.hub, entry, 0, "small"));
            let title = widgets::show_title(&entry.title, "row");
            title.set_lines(2);
            title.set_ellipsize(gtk::pango::EllipsizeMode::End);
            cell.append(&title);
            cell.append(&episode_line(broadcast, now));
        }
        opens(&cell, broadcast)
    }

    fn show_agenda(&self, days: &[glib::DateTime], broadcasts: &[Broadcast]) {
        clear(&self.agenda);
        let now = now();
        let today = glib::DateTime::now_local().ok();
        let narrow = self.ctx.size() == Size::Narrow;
        // Today first, then the days after it, then the days of the week already gone.
        let at =
            today.as_ref().and_then(|t| days.iter().position(|d| same_day(t, d))).unwrap_or(0);
        for day in days[at..].iter().chain(&days[..at]) {
            let on_day: Vec<&Broadcast> =
                broadcasts.iter().filter(|b| same_day(&local(b.airs), day)).collect();
            if on_day.is_empty() {
                continue;
            }
            let is_today = today.as_ref().is_some_and(|t| same_day(t, day));
            let name = day_heading(day, month_first());
            let heading =
                widgets::heading(&if is_today { format!("Today, {name}") } else { name });
            let list = gtk::ListBox::new();
            list.add_css_class("boxed-list");
            list.set_selection_mode(gtk::SelectionMode::None);
            for broadcast in on_day {
                list.append(&self.agenda_row(broadcast, now, narrow));
            }
            let section = gtk::Box::new(gtk::Orientation::Vertical, 8);
            section.append(&heading);
            section.append(&list);
            self.agenda.append(&section);
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
        text.append(&episode_line(broadcast, now));
        line.append(&text);
        if broadcast.watched() {
            line.add_css_class("dim-label");
            let done = gtk::Image::from_icon_name("object-select-symbolic");
            done.set_tooltip_text(Some("Watched"));
            line.append(&done);
        }
        let row = gtk::ListBoxRow::builder().child(&line).activatable(true).build();
        row.set_action_name(Some("win.open-title"));
        row.set_action_target_value(Some(&entry.id.get().to_variant()));
        widgets::title_menu(&row, entry.id.get(), entry.list_status.is_none());
        row
    }
}

/// The episode line, with the water dot before it when it is out and not watched.
fn episode_line(broadcast: &Broadcast, now: i64) -> gtk::Box {
    let line = gtk::Box::new(gtk::Orientation::Horizontal, 6);
    if broadcast.is_new(now) {
        let dot = gtk::Box::new(gtk::Orientation::Horizontal, 0);
        dot.add_css_class("state-dot");
        dot.set_valign(gtk::Align::Center);
        dot.set_tooltip_text(Some("Out, not watched"));
        line.append(&dot);
    }
    let text = widgets::haze(&broadcast.line(), "caption");
    text.add_css_class("numeric");
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
    button.set_tooltip_text(Some(&entry.title));
    button.update_property(&[gtk::accessible::Property::Label(&format!(
        "{}, {}",
        entry.title,
        broadcast.line()
    ))]);
    widgets::title_menu(&button, id, entry.list_status.is_none());
    button.upcast()
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
fn twelve_hour() -> bool {
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
    let answer = ask("ReadOne").or_else(|| ask("Read"))?;
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
        assert!(broadcast.line().starts_with("Ep 10 · "));
        broadcast.was = Some(at - 3600);
        assert!(broadcast.line().contains("(was "));
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
        assert!(!broadcast.line().ends_with("Dub") && !broadcast.line().ends_with("Sub"));
        broadcast.entry.airing_approximate = true;
        broadcast.entry.airing_translation = Some(Translation::Dub);
        assert!(broadcast.line().starts_with("Ep 3 · ~"));
        assert!(broadcast.line().ends_with(" · Dub"));
    }
}
