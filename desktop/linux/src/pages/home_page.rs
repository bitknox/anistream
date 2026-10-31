//! Home: a spotlight over shelves. The spotlight is one title with its main action in amber;
//! under it the shelves the engine chose, Continue first. Every shelf is a horizontal
//! scrolled window with paging buttons that fade in while the pointer is over it. The engine
//! sends Home whole and again as each part lands; a shelf that changes is updated in place, so it
//! keeps its scroll, a card that leaves fades and its neighbours close the gap, and one that comes
//! back gets its room first.

use std::{
    cell::{Cell, RefCell},
    rc::{Rc, Weak},
    time::{Duration, Instant},
};

use adw::prelude::*;
use anistream_core::ids::AnilistId;
use anistream_i18n::t;
use anistream_model::{
    catalog::Entry,
    command::Command,
    event::Event,
    home::{
        HomeAction, HomeEnd, HomeItem, HomeLink, HomePart, HomeSection, HomeView, Playable,
        SectionKind, Spotlight, SpotlightCase,
    },
};
use gtk::{gio, glib};

use crate::{
    motion::{self, Fader},
    nagare, say,
    squeeze::Squeeze,
    states, widgets,
    window::Ctx,
};

/// The column Home keeps to.
const COLUMN: i32 = 990;
/// How far the paging buttons stand past the column's edge: their centres sit on it. Every part
/// of the column is inset by this much, so the buttons have room.
const EDGE: i32 = 18;
/// The spotlight's art takes this share of the column, 560 of 990.
const ART_SHARE: f64 = 560.0 / 990.0;
const SPOT_GAP: i32 = 36;
/// Below this width the spotlight's art goes above its text.
const STACKED: i32 = 600;
/// A Continue card's width, 16:9.
const CARD_WIDTH: i32 = 272;
/// A poster card's width, 2:3.
const POSTER_WIDTH: i32 = 128;
const GAP: i32 = 16;
/// Space between shelves, and above the closing line.
const BETWEEN: i32 = 36;
const ABOVE_END: i32 = 28;
/// This week: the days it shows, the titles a day shows before "n more", its covers' width, and
/// the least width of a day.
const DAYS: u32 = 7;
const DAY_SHOWN: usize = 3;
const DAY_COVER: i32 = 40;
const DAY_WIDTH: i32 = 120;
/// How long a wait is silent before placeholders show.
const SILENT: Duration = Duration::from_millis(600);
/// When the activity line joins the capsule.
const ACTIVITY: Duration = Duration::from_secs(4);

/// What a slot of the column shows.
enum Make {
    Spotlight,
    Shelf(HomeSection),
    Week(HomeSection),
    Placeholder(HomePart, bool),
    End(HomeEnd),
}

struct Slot {
    key: String,
    signature: String,
    /// Its place in the column, which closes when it leaves.
    place: Squeeze,
    shelf: Option<Rc<Shelf>>,
}

struct Page {
    ctx: Rc<Ctx>,
    stack: gtk::Stack,
    column: gtk::Box,
    spot: gtk::Stack,
    spot_signature: RefCell<String>,
    spot_key: RefCell<Option<(AnilistId, SpotlightCase)>>,
    slots: RefCell<Vec<Slot>>,
    capsule: gtk::Box,
    capsule_text: gtk::Label,
    capsule_spin: gtk::Spinner,
    last: RefCell<Option<HomeView>>,
    /// Whether placeholders are on screen, and since when.
    placeholders: Cell<Option<Instant>>,
    silent_timer: RefCell<Option<glib::SourceId>>,
    activity_timer: RefCell<Option<glib::SourceId>>,
    activity: Cell<bool>,
    failed_told: Cell<bool>,
    /// Everything failed and nothing is kept: the failure screen shows until something loads.
    failed_empty: Cell<bool>,
    probed: Cell<Option<AnilistId>>,
    /// The app's offline banner, as Home words it.
    banner: adw::Banner,
}

pub fn build(ctx: &Rc<Ctx>) -> adw::NavigationPage {
    let header = adw::HeaderBar::new();
    header.pack_start(&widgets::sidebar_button(&ctx.split));
    header.pack_start(&crate::window::tv_session_button(ctx));

    // Says only what the sidebar does not: offline since a time, or still loading.
    let capsule = gtk::Box::new(gtk::Orientation::Horizontal, 6);
    capsule.add_css_class("home-capsule");
    let capsule_spin = gtk::Spinner::new();
    capsule_spin.set_visible(false);
    let capsule_text = gtk::Label::new(None);
    capsule_text.add_css_class("caption");
    capsule_text.add_css_class("haze");
    capsule.append(&capsule_spin);
    capsule.append(&capsule_text);
    capsule.set_visible(false);
    header.pack_end(&capsule);

    let column = gtk::Box::new(gtk::Orientation::Vertical, 0);
    column.set_margin_top(12);
    column.set_margin_bottom(36);
    column.set_margin_start(6);
    column.set_margin_end(6);
    let clamp = adw::Clamp::builder()
        .maximum_size(COLUMN + 2 * EDGE)
        .tightening_threshold(COLUMN + 2 * EDGE)
        .child(&column)
        .build();
    let scroll = gtk::ScrolledWindow::builder()
        .hscrollbar_policy(gtk::PolicyType::Never)
        .vexpand(true)
        .child(&clamp)
        .build();

    let open = gtk::Button::with_label(&t::home_open_downloads());
    open.add_css_class("pill");
    open.set_action_name(Some("win.go"));
    open.set_action_target_value(Some(&"downloads".to_variant()));
    let offline_empty = nagare::status_page(
        &t::home_offline_empty_title(),
        &t::home_offline_empty_body(),
        Some(&open),
    );

    let again = gtk::Button::with_label(&t::home_try_again());
    again.add_css_class("pill");
    again.set_halign(gtk::Align::Center);
    again.set_action_name(Some("win.home-reload"));
    let failed = adw::StatusPage::builder()
        .title(t::home_failed())
        .icon_name("dialog-warning-symbolic")
        .child(&again)
        .build();
    failed.add_css_class("compact");
    failed.add_css_class("failed");

    let stack = gtk::Stack::new();
    stack.set_transition_type(gtk::StackTransitionType::Crossfade);
    stack.set_transition_duration(200);
    stack.add_named(&gtk::Box::new(gtk::Orientation::Vertical, 0), Some("blank"));
    stack.add_named(&scroll, Some("content"));
    stack.add_named(&offline_empty, Some("offline"));
    stack.add_named(&failed, Some("failed"));
    stack.set_visible_child_name("blank");

    let spot = gtk::Stack::new();
    spot.set_transition_type(gtk::StackTransitionType::Crossfade);
    spot.set_transition_duration(300);
    spot.set_hhomogeneous(false);
    spot.set_vhomogeneous(false);
    spot.set_interpolate_size(false);
    spot.set_margin_start(EDGE);
    spot.set_margin_end(EDGE);

    let page = Rc::new(Page {
        ctx: Rc::clone(ctx),
        stack,
        column,
        spot,
        spot_signature: RefCell::new(String::new()),
        spot_key: RefCell::new(None),
        slots: RefCell::new(Vec::new()),
        capsule,
        capsule_text,
        capsule_spin,
        last: RefCell::new(None),
        placeholders: Cell::new(None),
        silent_timer: RefCell::new(None),
        activity_timer: RefCell::new(None),
        activity: Cell::new(false),
        failed_told: Cell::new(false),
        failed_empty: Cell::new(false),
        probed: Cell::new(None),
        banner: widgets::offline_banner(),
    });

    let reload = gio::SimpleAction::new("home-reload", None);
    reload.connect_activate({
        let hub = Rc::clone(&ctx.hub);
        move |_, _| hub.send(Command::LoadHome { refresh: true })
    });
    ctx.window.add_action(&reload);

    let dismiss = gio::SimpleAction::new("home-dismiss", Some(glib::VariantTy::UINT32));
    dismiss.connect_activate({
        let page = Rc::downgrade(&page);
        move |_, arg| {
            if let (Some(page), Some(id)) = (page.upgrade(), arg.and_then(u32::from_variant)) {
                page.dismiss(AnilistId::new(id));
            }
        }
    });
    ctx.window.add_action(&dismiss);

    // Mark as watched from the spotlight's More menu, with Undo in its notice.
    let mark = gio::SimpleAction::new(
        "home-mark-watched",
        Some(glib::VariantTy::new("(us)").expect("a type")),
    );
    mark.connect_activate({
        let ctx = Rc::downgrade(ctx);
        move |_, arg| {
            let (Some(ctx), Some((id, episode))) =
                (ctx.upgrade(), arg.and_then(<(u32, String)>::from_variant))
            else {
                return;
            };
            let target = (id, episode.clone()).to_variant();
            let _ = WidgetExt::activate_action(&ctx.window, "win.mark-watched", Some(&target));
            let number = say::episode_n(&episode);
            ctx.toast_to(
                &t::home_marked_watched(number),
                &t::action_undo(),
                "win.home-unmark",
                &format!("{id}/{episode}"), // i18n-skip
            );
        }
    });
    ctx.window.add_action(&mark);
    let unmark = gio::SimpleAction::new("home-unmark", Some(glib::VariantTy::STRING));
    unmark.connect_activate({
        let ctx = Rc::downgrade(ctx);
        move |_, arg| {
            let Some(ctx) = ctx.upgrade() else { return };
            let Some(text) = arg.and_then(String::from_variant) else { return };
            if let Some((id, episode)) = text.split_once('/')
                && let Ok(id) = id.parse::<u32>()
            {
                let target = (id, episode.to_string()).to_variant();
                let _ = WidgetExt::activate_action(
                    &ctx.window,
                    "win.mark-unwatched",
                    Some(&target),
                );
            }
        }
    });
    ctx.window.add_action(&unmark);

    let listener = ctx.hub.listen({
        let page = Rc::downgrade(&page);
        move |event| {
            if let (Some(page), Event::Home(view)) = (page.upgrade(), event) {
                page.apply(view);
            }
        }
    });
    // The page as `widgets::content_page` makes it, with Home's own hold on the banner.
    let view = adw::ToolbarView::new();
    view.add_top_bar(&header);
    view.add_top_bar(&page.banner);
    view.set_content(Some(&page.stack));
    let nav = adw::NavigationPage::builder().title(t::home_title()).child(&view).build();
    widgets::keep(&nav, (Rc::clone(&page), ForgetOnDrop(Rc::clone(&ctx.hub), listener)));
    ctx.hub.send(Command::LoadHome { refresh: false });
    nav
}

/// Stops listening when the page goes.
pub struct ForgetOnDrop(pub Rc<crate::hub::Hub>, pub u64);

impl Drop for ForgetOnDrop {
    fn drop(&mut self) {
        self.0.forget(self.1);
    }
}

impl Page {
    fn apply(self: &Rc<Self>, view: &HomeView) {
        self.last.replace(Some(view.clone()));
        let empty = view.spotlight.is_none() && view.sections.is_empty();
        self.watch_pending(view);
        self.capsule_state(view);
        if !empty || !view.pending.is_empty() {
            self.failed_empty.set(false);
        }
        if empty && view.pending.is_empty() {
            if view.offline {
                self.stack.set_visible_child_name("offline");
                return;
            }
            // Nothing loaded and nothing kept: the failure screen says so, with Try again,
            // instead of a closing line on an empty page.
            if view.failed || self.failed_empty.get() {
                self.failed_empty.set(true);
                self.failed_told.set(true);
                self.stack.set_visible_child_name("failed");
                return;
            }
        }
        self.tell_failure(view);
        if empty && !view.pending.is_empty() && self.placeholders.get().is_none() {
            // The first 600 ms are silent: the ground, then the shape of what comes.
            if self.stack.visible_child_name().as_deref() != Some("content") {
                self.stack.set_visible_child_name("blank");
            }
        } else {
            self.stack.set_visible_child_name("content");
        }
        self.rebuild(view);
    }

    /// The silent wait, the placeholders after it, and the activity line after four seconds.
    fn watch_pending(self: &Rc<Self>, view: &HomeView) {
        let waiting = !view.pending.is_empty();
        if waiting {
            if self.silent_timer.borrow().is_none() && self.placeholders.get().is_none() {
                let page = Rc::downgrade(self);
                let id = glib::timeout_add_local_once(SILENT, move || {
                    if let Some(page) = page.upgrade() {
                        page.silent_timer.replace(None);
                        page.placeholders.set(Some(Instant::now()));
                        if page.stack.visible_child_name().as_deref() == Some("blank") {
                            page.stack.set_visible_child_name("content");
                        }
                        page.redraw();
                    }
                });
                self.silent_timer.replace(Some(id));
            }
            if self.activity_timer.borrow().is_none() && !self.activity.get() {
                let page = Rc::downgrade(self);
                let id = glib::timeout_add_local_once(ACTIVITY, move || {
                    if let Some(page) = page.upgrade() {
                        page.activity_timer.replace(None);
                        page.activity.set(true);
                        page.redraw();
                    }
                });
                self.activity_timer.replace(Some(id));
            }
        } else {
            if let Some(id) = self.silent_timer.take() {
                id.remove();
            }
            if let Some(id) = self.activity_timer.take() {
                id.remove();
            }
            self.activity.set(false);
            // Placeholders stay at least 0.3 s once shown, so they never flash.
            if let Some(since) = self.placeholders.get() {
                let left = states::AT_LEAST.saturating_sub(since.elapsed());
                if left.is_zero() {
                    self.placeholders.set(None);
                } else {
                    let page = Rc::downgrade(self);
                    glib::timeout_add_local_once(left, move || {
                        if let Some(page) = page.upgrade() {
                            let still = page
                                .last
                                .borrow()
                                .as_ref()
                                .is_some_and(|v| !v.pending.is_empty());
                            if !still {
                                page.placeholders.set(None);
                                page.redraw();
                            }
                        }
                    });
                }
            }
        }
    }

    fn redraw(self: &Rc<Self>) {
        let view = self.last.borrow().clone();
        if let Some(view) = view {
            self.capsule_state(&view);
            self.rebuild(&view);
        }
    }

    /// The capsule in the header says only that suggestions are still loading: synced is the
    /// sidebar's to say, and offline the banner's. On Home the banner says since when Home
    /// is from.
    fn capsule_state(&self, view: &HomeView) {
        self.capsule_spin.set_visible(false);
        self.capsule_spin.set_spinning(false);
        let kept = view.spotlight.is_some() || !view.sections.is_empty();
        let since = view
            .offline_since
            .filter(|_| view.offline && kept)
            .and_then(|s| glib::DateTime::from_unix_local(s).ok())
            .and_then(|d| d.format("%H:%M").ok())
            .map(|at| t::home_offline(at.as_str()));
        self.banner.set_title(&since.unwrap_or_else(t::state_offline_line));
        let text = if !view.offline && self.activity.get() && !view.pending.is_empty() {
            self.capsule_spin.set_visible(true);
            self.capsule_spin.set_spinning(true);
            Some(t::home_loading())
        } else {
            None
        };
        match text {
            Some(text) => {
                self.capsule_text.set_label(&text);
                if !self.capsule.is_visible() {
                    self.capsule.set_visible(true);
                    motion::fade_in(&self.capsule);
                }
            }
            None => self.capsule.set_visible(false),
        }
    }

    /// One notice with Try again when the suggestions failed; what loaded stays.
    fn tell_failure(&self, view: &HomeView) {
        if view.failed && !view.offline {
            if !self.failed_told.replace(true) {
                self.ctx.toast_with(&t::home_failed(), &t::home_try_again(), "win.home-reload");
            }
        } else {
            self.failed_told.set(false);
        }
    }

    fn rebuild(self: &Rc<Self>, view: &HomeView) {
        let season = view.season.map(|s| say::season(s.season, s.year));
        let mut wanted: Vec<(String, String, Make)> = Vec::new();

        if let Some(spotlight) = &view.spotlight {
            let before = self.spot_key.borrow().is_some();
            self.set_spotlight(spotlight, before);
            wanted.push(("spotlight".into(), String::new(), Make::Spotlight));
        } else {
            self.spot_signature.replace(String::new());
            self.spot_key.replace(None);
        }
        for section in &view.sections {
            let key = format!("{:?}", section.kind);
            let aside =
                if section.kind == SectionKind::ThisSeason { season.as_deref() } else { None };
            let signature = format!("{section:?}{aside:?}");
            let make = if section.kind == SectionKind::ThisWeek {
                Make::Week(section.clone())
            } else {
                Make::Shelf(section.clone())
            };
            wanted.push((key, signature, make));
        }
        if self.placeholders.get().is_some() {
            for (n, part) in view.pending.iter().enumerate() {
                let first = n == 0 && view.spotlight.is_none() && view.sections.is_empty();
                wanted.push((
                    format!("ph-{part:?}"),
                    String::new(),
                    Make::Placeholder(*part, first),
                ));
            }
        }
        if let Some(end) = view.end.filter(|_| view.pending.is_empty()) {
            wanted.push(("end".into(), format!("{end:?}"), Make::End(end)));
        }

        // A place that finished closing is gone from the column; forget it.
        let mut old: Vec<Slot> = self.slots.take();
        old.retain(|s| s.place.parent().is_some());
        let first_build = old.is_empty();
        let had_placeholders = old.iter().any(|s| s.key.starts_with("ph-"));
        let before: Vec<Squeeze> = old.iter().map(|s| s.place.clone()).collect();
        let mut slots: Vec<Slot> = Vec::new();
        // Slots made this time, and whether they arrive among what was there.
        let mut fresh: Vec<usize> = Vec::new();
        for (key, signature, make) in wanted {
            if let Some(mut slot) = old.iter().position(|s| s.key == key).map(|i| old.remove(i))
            {
                let reopen = slot.place.is_closing();
                if (slot.signature != signature || reopen) && key != "spotlight" {
                    match (&slot.shelf, &make) {
                        (Some(shelf), Make::Shelf(section)) if !reopen => {
                            let aside = (section.kind == SectionKind::ThisSeason)
                                .then(|| season.clone());
                            shelf.update(section, aside.flatten());
                        }
                        _ => {
                            let (widget, shelf) = self.make(&make, season.as_deref());
                            slot.place.set_child(&widget);
                            slot.shelf = shelf;
                        }
                    }
                    slot.signature = signature;
                }
                if reopen {
                    // Back while its place closed: the room opens again, then it fades in.
                    let child = slot.place.child();
                    if let Some(child) = &child {
                        child.set_opacity(0.0);
                    }
                    slot.place.open(move || {
                        if let Some(child) = &child {
                            motion::fade_in(child);
                        }
                    });
                }
                slots.push(slot);
                continue;
            }
            let (widget, shelf) = self.make(&make, season.as_deref());
            if let Some(gone) = widget.parent().and_downcast::<Squeeze>() {
                // The spotlight is one widget, still held by the place it left.
                gone.set_child(&gtk::Box::new(gtk::Orientation::Vertical, 0));
            }
            let place = Squeeze::new(&widget, gtk::Orientation::Vertical, 0);
            if !first_build && !had_placeholders && !key.starts_with("ph-") {
                fresh.push(slots.len());
            }
            slots.push(Slot { key, signature, place, shelf });
        }
        // A section arriving above others makes its room on the standard token, then fades in;
        // one arriving at the end fades in where it lands. Placeholders giving way to content
        // are one crossfade, with no reflow.
        for &n in &fresh {
            let Some(widget) = slots[n].place.child() else { continue };
            let among = slots[n + 1..].iter().any(|s| before.contains(&s.place));
            if among {
                slots[n].place.set_closed();
                widget.set_opacity(0.0);
                slots[n].place.open(move || {
                    motion::fade_in(&widget);
                });
            } else {
                widget.set_opacity(0.0);
                let shown = widget.clone();
                motion::once_mapped(&widget, move || {
                    motion::crossfade_in(&shown);
                });
            }
        }
        if had_placeholders {
            for slot in slots.iter().filter(|s| !before.contains(&s.place)) {
                if let Some(widget) = slot.place.child() {
                    widget.set_opacity(0.0);
                    let shown = widget.clone();
                    motion::once_mapped(&widget, move || {
                        motion::crossfade_in(&shown);
                    });
                }
            }
        }

        // What left: placeholders give way at once to the crossfade; a section fades out, then
        // its place closes so the shelves below close the gap.
        let mut leaving: Vec<Squeeze> = Vec::new();
        for slot in old {
            if slot.key.starts_with("ph-") || had_placeholders {
                continue;
            }
            if !slot.place.is_closing()
                && let Some(child) = slot.place.child()
            {
                let place = slot.place.clone();
                let column = self.column.clone();
                place.mark_closing();
                motion::fade_away(&child, move || {
                    if !place.is_closing() {
                        return;
                    }
                    let gone = place.clone();
                    let column = column.clone();
                    place.close(move || {
                        if gone.parent().as_ref() == Some(column.upcast_ref()) {
                            column.remove(&gone);
                        }
                    });
                });
            }
            // Off screen a section goes at once, and is already out of the column.
            if slot.place.parent().is_some() {
                leaving.push(slot.place.clone());
                slots.push(slot);
            }
        }

        // Lay the column out in order, each leaving place where it was.
        let live: Vec<Squeeze> = slots
            .iter()
            .filter(|s| !leaving.contains(&s.place))
            .map(|s| s.place.clone())
            .collect();
        let order = keep_in_place(&before, live, &leaving);
        let mut child = self.column.first_child();
        while let Some(c) = child {
            child = c.next_sibling();
            if !order.iter().any(|p| p.upcast_ref::<gtk::Widget>() == &c) {
                self.column.remove(&c);
            }
        }
        let mut previous: Option<gtk::Widget> = None;
        for (n, place) in order.iter().enumerate() {
            if let Some(child) = place.child() {
                let key = slots.iter().find(|s| &s.place == place).map(|s| s.key.as_str());
                child.set_margin_top(match (n, key) {
                    (0, _) => 0,
                    (_, Some("end")) => ABOVE_END,
                    _ => BETWEEN,
                });
            }
            if place.parent().as_ref() == Some(self.column.upcast_ref()) {
                self.column.reorder_child_after(place, previous.as_ref());
            } else {
                self.column.insert_child_after(place, previous.as_ref());
            }
            previous = Some(place.clone().upcast());
        }
        self.slots.replace(slots);
    }

    fn make(
        self: &Rc<Self>,
        make: &Make,
        season: Option<&str>,
    ) -> (gtk::Widget, Option<Rc<Shelf>>) {
        let inset = |w: gtk::Widget| {
            w.set_margin_start(EDGE);
            w.set_margin_end(EDGE);
            (w, None)
        };
        match make {
            Make::Spotlight => (self.spot.clone().upcast(), None),
            Make::Shelf(section) => {
                let aside =
                    (section.kind == SectionKind::ThisSeason).then_some(season).flatten();
                let shelf = Shelf::new(self, section, aside.map(str::to_string));
                (shelf.outer.clone().upcast(), Some(shelf))
            }
            Make::Week(section) => inset(self.week(section)),
            Make::Placeholder(part, first) => inset(placeholder_shelf(*part, *first)),
            Make::End(end) => inset(self.closing(*end)),
        }
    }

    fn set_spotlight(self: &Rc<Self>, spotlight: &Spotlight, shown: bool) {
        let signature = format!("{spotlight:?}");
        if *self.spot_signature.borrow() == signature {
            return;
        }
        self.spot_signature.replace(signature);
        let key = (spotlight.entry.id, spotlight.case);
        let changed = self.spot_key.replace(Some(key)) != Some(key);
        if changed && self.probed.replace(Some(key.0)) != Some(key.0) {
            self.ctx.hub.send(Command::ProbeTitle { id: key.0 });
        }
        let widget = self.spotlight(spotlight);
        let old = self.spot.first_child();
        // A new title or case crossfades in place; a new line under the same one does not move.
        self.spot.set_transition_type(if changed && shown && old.is_some() {
            gtk::StackTransitionType::Crossfade
        } else {
            gtk::StackTransitionType::None
        });
        let name = format!("{}-{:?}-{}", key.0.get(), key.1, self.spot.pages().n_items());
        self.spot.add_named(&widget, Some(&name));
        self.spot.set_visible_child(&widget);
        let spot = self.spot.clone();
        let gone: Vec<gtk::Widget> = {
            let mut all = Vec::new();
            let mut next = spot.first_child();
            while let Some(child) = next {
                next = child.next_sibling();
                if child != widget {
                    all.push(child);
                }
            }
            all
        };
        glib::timeout_add_local_once(Duration::from_millis(400), move || {
            for child in gone {
                if child.parent().as_ref() == Some(spot.upcast_ref()) {
                    spot.remove(&child);
                }
            }
        });
    }

    fn spotlight(self: &Rc<Self>, s: &Spotlight) -> gtk::Widget {
        let info = gtk::Box::new(gtk::Orientation::Vertical, 0);

        let label = gtk::Label::new(Some(&say::msg(&s.label)));
        label.add_css_class("haze");
        label.add_css_class("home-label");
        label.set_xalign(0.0);
        label.set_margin_bottom(8);
        info.append(&label);

        let title = widgets::show_title(&s.entry.title, "home-spot-title");
        title.set_xalign(0.0);
        title.set_wrap(true);
        title.set_lines(2);
        title.set_ellipsize(gtk::pango::EllipsizeMode::End);
        title.set_natural_wrap_mode(gtk::NaturalWrapMode::Word);
        info.append(&title);

        if let Some(other) = s.entry.secondary.as_deref().filter(|o| *o != s.entry.title) {
            info.append(&line(other, "haze", 4));
        }
        let facts = say::meta_line(&s.entry);
        if !facts.is_empty() {
            info.append(&line(&facts, "home-facts", 10));
        }
        if !s.entry.synopsis.is_empty() {
            let synopsis = gtk::Label::new(Some(s.entry.synopsis.trim()));
            synopsis.add_css_class("haze");
            synopsis.set_xalign(0.0);
            synopsis.set_wrap(true);
            synopsis.set_lines(3);
            synopsis.set_ellipsize(gtk::pango::EllipsizeMode::End);
            synopsis.set_max_width_chars(52);
            synopsis.set_margin_top(12);
            info.append(&synopsis);
        }
        if let Some(text) = s.line.as_ref().map(say::msg) {
            let row = gtk::Box::new(gtk::Orientation::Horizontal, 12);
            row.set_margin_top(16);
            if let Some(fraction) = s.resume.and_then(|r| r.fraction) {
                row.append(&progress_bar(fraction, 200));
            }
            let l = gtk::Label::new(Some(&text));
            l.add_css_class("haze");
            row.append(&l);
            info.append(&row);
        }

        let not_found = s.playable == Playable::NotFound
            && matches!(s.action, HomeAction::Resume | HomeAction::Play);
        let id = s.entry.id.get();
        let actions = gtk::Box::new(gtk::Orientation::Horizontal, 10);
        actions.set_margin_top(18);
        let main = match (s.action, not_found) {
            (HomeAction::Resume, false) => {
                Some((t::home_resume_episode(i64::from(s.episode)), play_target(id, s.episode)))
            }
            (HomeAction::Play, false) => {
                Some((t::home_play_episode(i64::from(s.episode)), play_target(id, s.episode)))
            }
            (HomeAction::AddToPlanning, _) | (_, true) => {
                Some((t::up_next_add_planning(), ("win.plan".to_string(), id.to_variant())))
            }
            (HomeAction::Nothing, false) => None,
        };
        if let Some((text, (action, target))) = main {
            let b = gtk::Button::with_label(&text);
            b.add_css_class("suggested-action");
            b.add_css_class("home-main");
            b.set_action_name(Some(&action));
            b.set_action_target_value(Some(&target));
            actions.append(&b);
        }
        let details = gtk::Button::with_label(&t::home_details());
        details.add_css_class("home-sec");
        details.set_action_name(Some("win.open-title"));
        details.set_action_target_value(Some(&id.to_variant()));
        actions.append(&details);
        let more = gtk::MenuButton::new();
        more.set_icon_name("view-more-symbolic");
        more.add_css_class("home-more");
        more.set_tooltip_text(Some(&t::home_more()));
        let continuing = s.case == SpotlightCase::Continuing;
        more.set_menu_model(Some(&card_menu(
            &s.entry,
            if continuing { SectionKind::Continue } else { SectionKind::Starting },
            s.action,
            s.episode,
            s.playable,
            !continuing,
            continuing && s.action != HomeAction::AddToPlanning,
        )));
        actions.append(&more);
        info.append(&actions);
        if not_found {
            info.append(&line(&t::home_not_found(), "haze", 12));
        }

        let (frame, _) = widgets::art(
            &self.ctx.hub,
            s.entry.banner_url.as_deref().or(s.entry.cover_url.as_deref()),
            s.entry.cover_color.as_deref(),
            "spotart",
        );
        let art = crate::fit::Fit::new(&frame, 16.0 / 9.0, COLUMN, false, 0);
        art.update_property(&[gtk::accessible::Property::Label(&s.entry.title)]);

        let spot = spot_row::SpotRow::new(&info, &art);
        spot.add_css_class("home-spot");
        spot.upcast()
    }

    /// This week: seven days from today, up to three titles a day.
    fn week(self: &Rc<Self>, section: &HomeSection) -> gtk::Widget {
        let outer = gtk::Box::new(gtk::Orientation::Vertical, 12);
        outer.append(&heading_row(
            &say::msg(&section.title),
            None,
            section.view_all.as_ref().map(link),
        ));
        let grid = gtk::Grid::new();
        grid.set_column_homogeneous(true);
        grid.set_column_spacing(GAP as u32);
        grid.add_css_class("home-week");
        let today = glib::DateTime::now_local().ok();
        for day in 0..DAYS {
            let column = gtk::Box::new(gtk::Orientation::Vertical, 12);
            column.set_size_request(DAY_WIDTH, -1);
            let name = if day == 0 {
                t::home_today()
            } else {
                today
                    .as_ref()
                    .and_then(|d| d.add_days(day as i32).ok())
                    .and_then(|d| d.format("%A %-d").ok())
                    .map(|s| s.to_string())
                    .unwrap_or_default()
            };
            let name = gtk::Label::new(Some(&name));
            name.add_css_class("home-day");
            if day != 0 {
                name.add_css_class("haze");
            }
            name.set_xalign(0.0);
            name.set_ellipsize(gtk::pango::EllipsizeMode::End);
            column.append(&name);
            let items: Vec<&HomeItem> =
                section.items.iter().filter(|i| i.day == Some(day)).collect();
            if items.is_empty() {
                column.append(&line(&t::home_nothing_airing(), "haze", 0));
                column.last_child().inspect(|l| l.add_css_class("home-week-line"));
            }
            for item in items.iter().take(DAY_SHOWN) {
                column.append(&self.week_entry(item));
            }
            if items.len() > DAY_SHOWN {
                let more = gtk::Button::with_label(&t::home_day_more(
                    (items.len() - DAY_SHOWN) as i64,
                ));
                more.add_css_class("flat");
                more.add_css_class("home-day-more");
                more.set_halign(gtk::Align::Start);
                more.set_action_name(Some("win.go"));
                more.set_action_target_value(Some(&"calendar".to_variant()));
                column.append(&more);
            }
            grid.attach(&column, day as i32, 0, 1, 1);
        }
        // On a narrow window the week scrolls rather than squeezing its days.
        let scroll = gtk::ScrolledWindow::builder()
            .hscrollbar_policy(gtk::PolicyType::External)
            .vscrollbar_policy(gtk::PolicyType::Never)
            .child(&grid)
            .build();
        scroll.set_propagate_natural_height(true);
        outer.append(&scroll);
        outer.upcast()
    }

    fn week_entry(&self, item: &HomeItem) -> gtk::Widget {
        let row = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        let cover = widgets::cover(&self.ctx.hub, &item.entry, DAY_COVER, DAY_COVER * 3 / 2);
        cover.set_valign(gtk::Align::Start);
        row.append(&cover);
        let text = gtk::Box::new(gtk::Orientation::Vertical, 0);
        text.set_hexpand(true);
        let when = item
            .line
            .as_ref()
            .map(say::msg)
            .or_else(|| item.airs_at.map(crate::pages::calendar::time))
            .unwrap_or_default();
        let time = gtk::Box::new(gtk::Orientation::Horizontal, 6);
        if item.new {
            time.append(&new_dot());
        }
        let when = gtk::Label::new(Some(&when));
        when.add_css_class("home-week-time");
        when.set_xalign(0.0);
        when.set_ellipsize(gtk::pango::EllipsizeMode::End);
        when.set_max_width_chars(1);
        when.set_hexpand(true);
        time.append(&when);
        text.append(&time);
        let title = widgets::show_title(&item.entry.title, "home-week-title");
        title.set_xalign(0.0);
        title.set_wrap(true);
        title.set_natural_wrap_mode(gtk::NaturalWrapMode::Word);
        title.set_wrap_mode(gtk::pango::WrapMode::WordChar);
        title.set_lines(2);
        title.set_ellipsize(gtk::pango::EllipsizeMode::End);
        title.set_max_width_chars(10);
        text.append(&title);
        if let Some(episode) = item.episode {
            let episode = line(&t::episode_number(i64::from(episode)), "haze", 0);
            episode.add_css_class("home-week-line");
            episode.set_wrap(false);
            episode.set_ellipsize(gtk::pango::EllipsizeMode::End);
            episode.set_max_width_chars(1);
            text.append(&episode);
        }
        row.append(&text);

        let button = gtk::Button::new();
        button.set_child(Some(&row));
        button.add_css_class("flat");
        button.add_css_class("home-card");
        button.set_action_name(Some("win.open-title"));
        button.set_action_target_value(Some(&item.entry.id.get().to_variant()));
        button.update_property(&[gtk::accessible::Property::Label(&format!(
            "{}, {}",
            item.entry.title,
            when.label()
        ))]);
        widgets::context_menu(
            &button,
            &card_menu(
                &item.entry,
                SectionKind::ThisWeek,
                item.action,
                item.episode.unwrap_or(1),
                item.playable,
                false,
                false,
            ),
        );
        button.upcast()
    }

    fn card(&self, kind: SectionKind, item: &HomeItem) -> gtk::Widget {
        let continuing = kind == SectionKind::Continue;
        let width = if continuing { CARD_WIDTH } else { POSTER_WIDTH };
        let inner = gtk::Box::new(gtk::Orientation::Vertical, 0);
        inner.set_size_request(width, -1);

        if continuing {
            let (frame, _) = widgets::art(
                &self.ctx.hub,
                item.entry.banner_url.as_deref().or(item.entry.cover_url.as_deref()),
                item.entry.cover_color.as_deref(),
                "home-still",
            );
            let fit = crate::fit::Fit::new(&frame, 16.0 / 9.0, width, true, 0);
            inner.append(&fit);
            match item.progress {
                Some(p) => inner.append(&progress_bar(p, width)),
                None => {
                    let gap = gtk::Box::new(gtk::Orientation::Vertical, 0);
                    gap.set_size_request(-1, 3);
                    gap.set_margin_top(6);
                    inner.append(&gap);
                }
            }
        } else {
            let fit = widgets::cover(&self.ctx.hub, &item.entry, width, width * 3 / 2);
            fit.set_halign(gtk::Align::Start);
            inner.append(&fit);
        }

        // The words take the card's width and no more: a long title ends in an ellipsis rather
        // than widening its card.
        let title = widgets::show_title(
            &item.entry.title,
            if continuing { "home-card-title" } else { "home-poster-title" },
        );
        title.set_wrap(false);
        title.set_ellipsize(gtk::pango::EllipsizeMode::End);
        title.set_max_width_chars(1);
        title.set_margin_top(8);
        inner.append(&title);

        let text = item
            .line
            .as_ref()
            .map(say::msg)
            .or_else(|| (item.playable == Playable::NotFound).then(t::home_not_found))
            .or_else(|| item.starts.map(|d| t::up_next_starts(&say::fuzzy_date(d))))
            .or_else(|| item.airs_at.map(crate::pages::calendar::time))
            .unwrap_or_else(|| say::meta_line(&item.entry));
        let sub = gtk::Box::new(gtk::Orientation::Horizontal, 6);
        if item.new {
            sub.append(&new_dot());
        }
        let sub_text = gtk::Label::new(Some(&text));
        sub_text.add_css_class("haze");
        sub_text.add_css_class(if continuing { "home-card-sub" } else { "home-sub" });
        sub_text.set_xalign(0.0);
        sub_text.set_ellipsize(gtk::pango::EllipsizeMode::End);
        sub_text.set_max_width_chars(1);
        sub_text.set_hexpand(true);
        sub.append(&sub_text);
        inner.append(&sub);

        let button = gtk::Button::new();
        button.set_child(Some(&inner));
        button.add_css_class("flat");
        button.add_css_class("home-card");
        button.set_halign(gtk::Align::Start);
        button.set_valign(gtk::Align::Start);
        button.set_hexpand(false);
        button.set_action_name(Some("win.open-title"));
        button.set_action_target_value(Some(&item.entry.id.get().to_variant()));
        button.update_property(&[gtk::accessible::Property::Label(&format!(
            "{}, {}",
            item.entry.title, text
        ))]);
        let episode = item.episode.unwrap_or(1);
        let suggestion =
            matches!(kind, SectionKind::UpNext | SectionKind::Because | SectionKind::MoreGenre);
        if suggestion {
            button.add_css_class("home-suggestion");
        }
        widgets::context_menu(
            &button,
            &card_menu(
                &item.entry,
                kind,
                item.action,
                episode,
                item.playable,
                suggestion,
                false,
            ),
        );
        button.upcast()
    }

    /// The closing line, on the column's left edge under a faint rule. The names of the places it
    /// points to are links.
    fn closing(&self, end: HomeEnd) -> gtk::Widget {
        let column = gtk::Box::new(gtk::Orientation::Vertical, 16);
        let rule = gtk::Separator::new(gtk::Orientation::Horizontal);
        rule.add_css_class("home-rule");
        column.append(&rule);
        let (text, links, button) = match end {
            HomeEnd::Everything => (
                t::home_end(),
                vec![(t::nav_seasonal(), "seasonal"), (t::nav_calendar(), "calendar")],
                None,
            ),
            HomeEnd::SignIn => (
                t::home_end_signed_out(),
                Vec::new(),
                Some((t::home_sign_in(), "win.settings-pane", "accounts")),
            ),
            HomeEnd::NoSources => {
                (t::home_end_no_sources(), vec![(t::nav_plugin_stores(), "stores")], None)
            }
        };
        let row = gtk::Box::new(gtk::Orientation::Horizontal, 16);
        let l = gtk::Label::new(None);
        l.set_markup(&linked(&text, &links));
        l.add_css_class("haze");
        l.add_css_class("home-end");
        l.set_xalign(0.0);
        l.set_wrap(true);
        l.set_hexpand(button.is_none());
        l.connect_activate_link(|label, uri| {
            let _ = WidgetExt::activate_action(label, "win.go", Some(&uri.to_variant()));
            glib::Propagation::Stop
        });
        row.append(&l);
        if let Some((label, action, target)) = button {
            let b = gtk::Button::with_label(&label);
            b.add_css_class("home-sec");
            b.add_css_class("home-small");
            b.set_valign(gtk::Align::Center);
            b.set_action_name(Some(action));
            b.set_action_target_value(Some(&target.to_variant()));
            row.append(&b);
        }
        column.append(&row);
        column.upcast()
    }

    /// Not interested: the poster fades, its neighbours close the gap, and a notice offers
    /// Undo, which gives it its room back.
    fn dismiss(self: &Rc<Self>, id: AnilistId) {
        let entry = self.entry_shown(id).or_else(|| self.ctx.hub.entry(id));
        let title = entry.as_ref().map_or_else(String::new, |e| e.title.clone());
        let until_airs = entry.as_ref().is_some_and(|e| {
            e.airing_in.is_some_and(|s| s > 0) && e.airing_episode.is_none_or(|n| n <= 1)
        });
        for slot in self.slots.borrow().iter() {
            if let Some(shelf) = &slot.shelf {
                shelf.leave(id.get());
            }
        }
        self.ctx.hub.send(Command::DismissUpNext { id });
        let text = if until_airs {
            t::up_next_hidden_until_airs(&title)
        } else {
            t::up_next_hidden(&title)
        };
        self.ctx.toast_to(&text, &t::action_undo(), "win.restore-up-next", &id.to_string());
    }

    /// The title as Home shows it, from the spotlight or a shelf.
    fn entry_shown(&self, id: AnilistId) -> Option<Entry> {
        let last = self.last.borrow();
        let view = last.as_ref()?;
        view.spotlight
            .as_ref()
            .map(|s| &s.entry)
            .into_iter()
            .chain(view.sections.iter().flat_map(|s| s.items.iter().map(|i| &i.entry)))
            .find(|e| e.id == id)
            .cloned()
    }
}

/// A shelf: its heading, and a row of cards that keeps its place when the section changes.
struct Shelf {
    page: Weak<Page>,
    kind: SectionKind,
    outer: gtk::Box,
    title: gtk::Label,
    aside: gtk::Label,
    all: gtk::Button,
    all_label: gtk::Label,
    cards: gtk::Box,
    placed: RefCell<Vec<Placed>>,
}

/// A card in a shelf: its title's id, what it showed, and its place.
struct Placed {
    id: u32,
    signature: String,
    place: Squeeze,
}

impl Shelf {
    fn new(page: &Rc<Page>, section: &HomeSection, aside: Option<String>) -> Rc<Self> {
        let outer = gtk::Box::new(gtk::Orientation::Vertical, 12);
        let head = gtk::Box::new(gtk::Orientation::Horizontal, 10);
        head.set_margin_start(EDGE);
        head.set_margin_end(EDGE);
        let title = gtk::Label::new(None);
        title.add_css_class("home-heading");
        title.set_xalign(0.0);
        title.set_ellipsize(gtk::pango::EllipsizeMode::End);
        title.set_accessible_role(gtk::AccessibleRole::Heading);
        head.append(&title);
        let aside_label = gtk::Label::new(None);
        aside_label.add_css_class("haze");
        aside_label.add_css_class("home-aside");
        head.append(&aside_label);
        let spacer = gtk::Box::new(gtk::Orientation::Horizontal, 0);
        spacer.set_hexpand(true);
        head.append(&spacer);
        let (all, all_label) = view_all_button();
        head.append(&all);
        outer.append(&head);

        // Each card's place holds the gap after it, so a place that closes takes its gap along.
        let cards = gtk::Box::new(gtk::Orientation::Horizontal, 0);
        let scroll = gtk::ScrolledWindow::builder()
            .hscrollbar_policy(gtk::PolicyType::External)
            .vscrollbar_policy(gtk::PolicyType::Never)
            .child(&cards)
            .build();
        scroll.set_propagate_natural_height(true);
        scroll.set_margin_start(EDGE);
        scroll.set_margin_end(EDGE);
        let layers = gtk::Overlay::new();
        layers.set_child(Some(&scroll));
        let kind = section.kind;
        let (width, art_height) = if kind == SectionKind::Continue {
            (CARD_WIDTH, CARD_WIDTH * 9 / 16)
        } else {
            (POSTER_WIDTH, POSTER_WIDTH * 3 / 2)
        };
        let pitch = f64::from(width + GAP);
        let prev =
            arrow("go-previous-symbolic", &t::home_previous(), gtk::Align::Start, art_height);
        let next = arrow("go-next-symbolic", &t::home_next(), gtk::Align::End, art_height);
        layers.add_overlay(&prev);
        layers.add_overlay(&next);
        outer.append(&layers);

        let shelf = Rc::new(Self {
            page: Rc::downgrade(page),
            kind,
            outer: outer.clone(),
            title,
            aside: aside_label,
            all,
            all_label,
            cards,
            placed: RefCell::default(),
        });
        shelf.heading(section, aside);
        shelf.place(&section.items, false);

        let hadj = scroll.hadjustment();
        let over = Rc::new(Cell::new(false));
        let fade = Rc::new((Fader::default(), Fader::default()));
        let refresh = {
            let (hadj, prev, next, over, fade) =
                (hadj.clone(), prev.clone(), next.clone(), over.clone(), fade.clone());
            Rc::new(move || {
                let back = hadj.value() > 1.0;
                let forward = hadj.value() + hadj.page_size() < hadj.upper() - 1.0;
                fade.0.set(&prev, over.get() && back);
                fade.1.set(&next, over.get() && forward);
            })
        };
        hadj.connect_changed({
            let refresh = refresh.clone();
            move |_| refresh()
        });
        hadj.connect_value_changed({
            let refresh = refresh.clone();
            move |_| refresh()
        });
        let hover = Rc::new({
            let (over, refresh) = (over.clone(), refresh.clone());
            move |inside: bool| {
                over.set(inside);
                refresh();
            }
        });
        let motion_ctl = gtk::EventControllerMotion::new();
        motion_ctl.connect_enter({
            let hover = hover.clone();
            move |_, _, _| hover(true)
        });
        motion_ctl.connect_leave({
            let hover = hover.clone();
            move |_| hover(false)
        });
        layers.add_controller(motion_ctl);
        prev.connect_clicked({
            let scroll = scroll.clone();
            move |_| page_by(&scroll, -1, pitch)
        });
        next.connect_clicked({
            let scroll = scroll.clone();
            move |_| page_by(&scroll, 1, pitch)
        });
        // The pointer and the paging buttons as actions too, for the shots' motion captures.
        let group = gio::SimpleActionGroup::new();
        let hover_action = gio::SimpleAction::new("hover", Some(glib::VariantTy::BOOLEAN));
        hover_action.connect_activate({
            let hover = hover.clone();
            move |_, arg| hover(arg.and_then(bool::from_variant).unwrap_or(false))
        });
        group.add_action(&hover_action);
        let page_action = gio::SimpleAction::new("page", Some(glib::VariantTy::INT32));
        page_action.connect_activate({
            let scroll = scroll.clone();
            move |_, arg| page_by(&scroll, arg.and_then(i32::from_variant).unwrap_or(1), pitch)
        });
        group.add_action(&page_action);
        outer.insert_action_group("shelf", Some(&group));
        outer.add_css_class("home-shelf");
        widgets::keep(&outer, (fade, refresh, over, hover));
        shelf
    }

    fn heading(&self, section: &HomeSection, aside: Option<String>) {
        self.title.set_label(&say::msg(&section.title));
        let cut = section.total as usize > section.items.len();
        let aside = match self.kind {
            SectionKind::Continue if cut => {
                Some(t::home_continue_count(i64::from(section.total)))
            }
            _ => aside,
        };
        self.aside.set_label(aside.as_deref().unwrap_or_default());
        self.aside.set_visible(aside.is_some());
        // Continue's View all is there once the shelf is cut, past twelve.
        let link = section
            .view_all
            .as_ref()
            .filter(|_| self.kind != SectionKind::Continue || cut)
            .map(link);
        match link {
            Some((text, dest)) => {
                self.all_label.set_label(&text);
                self.all.set_action_target_value(Some(&dest.to_variant()));
                self.all.set_visible(true);
            }
            None => self.all.set_visible(false),
        }
    }

    /// The section changed: the heading follows, cards that left fade and close their gap, new
    /// ones get their room and fade in, and the rest stay where they are.
    fn update(self: &Rc<Self>, section: &HomeSection, aside: Option<String>) {
        self.heading(section, aside);
        self.place(&section.items, true);
    }

    fn place(self: &Rc<Self>, items: &[HomeItem], animate: bool) {
        let Some(page) = self.page.upgrade() else { return };
        let mut old = self.placed.take();
        let before: Vec<Squeeze> = old.iter().map(|p| p.place.clone()).collect();
        let mut live: Vec<Placed> = Vec::new();
        for item in items {
            let id = item.entry.id.get();
            let signature = format!("{item:?}");
            match old.iter().position(|p| p.id == id) {
                Some(n) => {
                    let mut placed = old.remove(n);
                    if placed.signature != signature {
                        placed.place.set_child(&page.card(self.kind, item));
                        placed.signature = signature;
                    }
                    if placed.place.is_closing() {
                        // Back, by Undo: room first, then the card.
                        let card = placed.place.child();
                        if let Some(card) = &card {
                            card.set_opacity(0.0);
                        }
                        placed.place.open(move || {
                            if let Some(card) = &card {
                                motion::fade_in(card);
                            }
                        });
                    }
                    live.push(placed);
                }
                None => {
                    let card = page.card(self.kind, item);
                    let place = Squeeze::new(&card, gtk::Orientation::Horizontal, GAP);
                    if animate {
                        place.set_closed();
                        card.set_opacity(0.0);
                        place.open(move || {
                            motion::fade_in(&card);
                        });
                    }
                    live.push(Placed { id, signature, place });
                }
            }
        }
        for placed in &old {
            if !placed.place.is_closing() {
                self.close(&placed.place);
            }
        }
        // Off screen a card goes at once, and is already out of the row.
        old.retain(|p| p.place.parent().is_some());
        let leaving: Vec<Squeeze> = old.iter().map(|p| p.place.clone()).collect();
        let order =
            keep_in_place(&before, live.iter().map(|p| p.place.clone()).collect(), &leaving);
        let mut previous: Option<gtk::Widget> = None;
        for place in &order {
            if place.parent().as_ref() == Some(self.cards.upcast_ref()) {
                self.cards.reorder_child_after(place, previous.as_ref());
            } else {
                self.cards.insert_child_after(place, previous.as_ref());
            }
            previous = Some(place.clone().upcast());
        }
        let mut placed: Vec<Placed> = live;
        placed.extend(old);
        placed.sort_by_key(|p| order.iter().position(|o| o == &p.place));
        self.placed.replace(placed);
    }

    /// The card for `id` leaves, if this shelf shows it.
    fn leave(self: &Rc<Self>, id: u32) {
        let place = self.placed.borrow().iter().find(|p| p.id == id).map(|p| p.place.clone());
        if let Some(place) = place.filter(|p| !p.is_closing()) {
            self.close(&place);
        }
    }

    /// Fade the card out, then close its place, then let it go.
    fn close(self: &Rc<Self>, place: &Squeeze) {
        let Some(card) = place.child() else { return };
        // Closing from now, so a second leave or an update does not start it again.
        place.mark_closing();
        let shelf = Rc::downgrade(self);
        let place = place.clone();
        motion::fade_away(&card, move || {
            let Some(shelf) = shelf.upgrade() else { return };
            // Asked back while it faded: its place is opening, and the card fades in after.
            if !place.is_closing() {
                return;
            }
            let (gone, weak) = (place.clone(), Rc::downgrade(&shelf));
            place.close(move || {
                if let Some(shelf) = weak.upgrade() {
                    shelf.cards.remove(&gone);
                    shelf.placed.borrow_mut().retain(|p| p.place != gone);
                }
            });
        });
    }
}

/// `live` in its new order, with each of `leaving` kept after whichever place came before it in
/// `before`, so a place that closes closes where it stood.
fn keep_in_place(before: &[Squeeze], live: Vec<Squeeze>, leaving: &[Squeeze]) -> Vec<Squeeze> {
    let mut order = live;
    for gone in leaving {
        let Some(at) = before.iter().position(|p| p == gone) else { continue };
        let after = before[..at].iter().rev().find_map(|p| order.iter().position(|o| o == p));
        order.insert(after.map_or(0, |n| n + 1), gone.clone());
    }
    order
}

/// The spotlight's row: the text on the left and the art on the right at 56% of the width, both
/// centred on each other; on a window too narrow for both side by side, the art above.
mod spot_row {
    use adw::subclass::prelude::*;
    use gtk::{glib, graphene, gsk, prelude::*};

    use super::{ART_SHARE, COLUMN, SPOT_GAP, STACKED};

    fn art_width(width: i32) -> i32 {
        (f64::from(width) * ART_SHARE).round() as i32
    }

    mod imp {
        use super::*;

        #[derive(Default)]
        pub struct SpotRow;

        #[glib::object_subclass]
        impl ObjectSubclass for SpotRow {
            const NAME: &'static str = "AnistreamSpotRow"; // i18n-skip
            type Type = super::SpotRow;
            type ParentType = gtk::Widget;
        }

        impl ObjectImpl for SpotRow {
            fn dispose(&self) {
                while let Some(child) = self.obj().first_child() {
                    child.unparent();
                }
            }
        }

        impl WidgetImpl for SpotRow {
            fn request_mode(&self) -> gtk::SizeRequestMode {
                gtk::SizeRequestMode::HeightForWidth
            }

            fn measure(
                &self,
                orientation: gtk::Orientation,
                for_size: i32,
            ) -> (i32, i32, i32, i32) {
                let Some((info, _)) = self.obj().parts() else { return (0, 0, -1, -1) };
                if orientation == gtk::Orientation::Horizontal {
                    let (min, _, _, _) = info.measure(orientation, -1);
                    return (min, COLUMN.max(min), -1, -1);
                }
                let width = if for_size < 0 { COLUMN } else { for_size };
                let height = if self.obj().stacked(&info, width) {
                    let (_, text, _, _) = info.measure(orientation, width);
                    width * 9 / 16 + 16 + text
                } else {
                    let art = art_width(width);
                    let (_, text, _, _) = info.measure(orientation, width - art - SPOT_GAP);
                    (art * 9 / 16).max(text)
                };
                (height, height, -1, -1)
            }

            fn size_allocate(&self, width: i32, height: i32, _baseline: i32) {
                let Some((info, art)) = self.obj().parts() else { return };
                let at = |x: i32, y: i32| {
                    Some(
                        gsk::Transform::new()
                            .translate(&graphene::Point::new(x as f32, y as f32)),
                    )
                };
                if self.obj().stacked(&info, width) {
                    let art_height = width * 9 / 16;
                    art.allocate(width, art_height, -1, at(0, 0));
                    let (_, text, _, _) = info.measure(gtk::Orientation::Vertical, width);
                    info.allocate(width, text, -1, at(0, art_height + 16));
                } else {
                    let art_w = art_width(width);
                    let art_h = art_w * 9 / 16;
                    let text_w = width - art_w - SPOT_GAP;
                    let (_, text, _, _) = info.measure(gtk::Orientation::Vertical, text_w);
                    info.allocate(text_w, text, -1, at(0, (height - text) / 2));
                    art.allocate(art_w, art_h, -1, at(width - art_w, (height - art_h) / 2));
                }
            }
        }
    }

    glib::wrapper! {
        pub struct SpotRow(ObjectSubclass<imp::SpotRow>)
            @extends gtk::Widget,
            @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget;
    }

    impl SpotRow {
        pub fn new(info: &impl IsA<gtk::Widget>, art: &impl IsA<gtk::Widget>) -> Self {
            let row: Self = glib::Object::new();
            info.set_parent(&row);
            art.set_parent(&row);
            row
        }

        fn parts(&self) -> Option<(gtk::Widget, gtk::Widget)> {
            Some((self.first_child()?, self.last_child()?))
        }

        fn stacked(&self, info: &gtk::Widget, width: i32) -> bool {
            let (min, _, _, _) = info.measure(gtk::Orientation::Horizontal, -1);
            width < STACKED || width - art_width(width) - SPOT_GAP < min
        }
    }
}

/// A heading row for a section built whole: its title, words beside it, and View all.
fn heading_row(
    title: &str,
    aside: Option<&str>,
    all: Option<(String, &'static str)>,
) -> gtk::Box {
    let head = gtk::Box::new(gtk::Orientation::Horizontal, 10);
    let label = gtk::Label::new(Some(title));
    label.add_css_class("home-heading");
    label.set_xalign(0.0);
    label.set_accessible_role(gtk::AccessibleRole::Heading);
    head.append(&label);
    if let Some(aside) = aside {
        let l = gtk::Label::new(Some(aside));
        l.add_css_class("haze");
        l.add_css_class("home-aside");
        head.append(&l);
    }
    let spacer = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    spacer.set_hexpand(true);
    head.append(&spacer);
    if let Some((text, dest)) = all {
        let (button, label) = view_all_button();
        label.set_label(&text);
        button.set_action_target_value(Some(&dest.to_variant()));
        head.append(&button);
    }
    head
}

/// View all and its kin: silk words with a chevron after them.
fn view_all_button() -> (gtk::Button, gtk::Label) {
    let label = gtk::Label::new(None);
    let chevron = gtk::Image::from_icon_name("go-next-symbolic");
    chevron.set_pixel_size(12);
    let inner = gtk::Box::new(gtk::Orientation::Horizontal, 4);
    inner.append(&label);
    inner.append(&chevron);
    let button = gtk::Button::new();
    button.set_child(Some(&inner));
    button.add_css_class("flat");
    button.add_css_class("home-all");
    button.set_valign(gtk::Align::Center);
    button.set_action_name(Some("win.go"));
    (button, label)
}

fn link(link: &HomeLink) -> (String, &'static str) {
    match link {
        HomeLink::Watching => (t::home_view_all(), "library"),
        HomeLink::SeasonalView => (t::home_seasonal(), "seasonal"),
        HomeLink::CalendarView => (t::home_full_calendar(), "calendar"),
        HomeLink::Browse { .. } => (t::home_search(), "search"),
    }
}

/// `text` as markup, the first time each name in `links` appears made a link to its place.
fn linked(text: &str, links: &[(String, &str)]) -> String {
    let mut markup = glib::markup_escape_text(text).to_string();
    for (name, dest) in links {
        let name = glib::markup_escape_text(name).to_string();
        if let Some(at) = markup.find(&name) {
            markup
                .replace_range(at..at + name.len(), &format!("<a href=\"{dest}\">{name}</a>")); // i18n-skip
        }
    }
    markup
}

fn new_dot() -> gtk::Box {
    let dot = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    dot.add_css_class("home-new");
    dot.set_valign(gtk::Align::Center);
    dot
}

fn line(text: &str, class: &str, top: i32) -> gtk::Label {
    let l = gtk::Label::new(Some(text));
    l.add_css_class(class);
    l.set_xalign(0.0);
    l.set_wrap(true);
    l.set_margin_top(top);
    l
}

fn play_target(id: u32, episode: u32) -> (String, glib::Variant) {
    ("win.play".to_string(), (id, episode.to_string()).to_variant())
}

fn progress_bar(fraction: f64, width: i32) -> gtk::ProgressBar {
    let bar = gtk::ProgressBar::new();
    bar.add_css_class("home-progress");
    bar.set_fraction(fraction);
    bar.set_size_request(width, -1);
    bar.set_halign(gtk::Align::Start);
    bar.set_valign(gtk::Align::Center);
    bar.set_margin_top(6);
    bar
}

/// A paging button, centred on the shelf's edge. Hidden from Tab and screen readers: the scroll
/// bar and the arrow keys do the same work.
fn arrow(icon: &str, tooltip: &str, side: gtk::Align, art_height: i32) -> gtk::Button {
    let b = gtk::Button::from_icon_name(icon);
    b.add_css_class("circular");
    b.add_css_class("home-arrow");
    b.set_halign(side);
    b.set_valign(gtk::Align::Start);
    b.set_margin_top(art_height / 2 - 18);
    b.set_size_request(36, 36);
    b.set_focusable(false);
    b.set_can_focus(false);
    b.set_tooltip_text(Some(tooltip));
    b.set_visible(false);
    b.update_property(&[gtk::accessible::Property::Label(tooltip)]);
    b
}

/// Page a shelf by the whole cards that fit, landing on a card's edge, on the standard token.
fn page_by(host: &gtk::ScrolledWindow, direction: i32, pitch: f64) {
    let adj = host.hadjustment();
    let fit = ((adj.page_size() + f64::from(GAP)) / pitch).floor().max(1.0);
    let at = (adj.value() / pitch).round();
    let end = (adj.upper() - adj.page_size()).max(adj.lower());
    let to = ((at + f64::from(direction) * fit) * pitch).clamp(adj.lower(), end);
    let target = adw::CallbackAnimationTarget::new({
        let adj = adj.clone();
        move |v| adj.set_value(v)
    });
    motion::standard(host, adj.value(), to, target).play();
}

/// The menu on a card and under the spotlight's More button. `mark` adds Mark as watched, for
/// the spotlight of a show being continued.
fn card_menu(
    entry: &Entry,
    kind: SectionKind,
    action: HomeAction,
    episode: u32,
    playable: Playable,
    suggestion: bool,
    mark: bool,
) -> gio::Menu {
    let id = entry.id.get();
    let menu = gio::Menu::new();
    let first = gio::Menu::new();
    let play = format!("(uint32 {id}, '{episode}')"); // i18n-skip
    if playable != Playable::NotFound {
        match action {
            HomeAction::Resume => first.append(
                Some(&t::home_resume_episode(i64::from(episode))),
                Some(&format!("win.play({play})")),
            ),
            HomeAction::Play => first.append(
                Some(&t::home_play_episode(i64::from(episode))),
                Some(&format!("win.play({play})")),
            ),
            _ => {}
        }
    }
    first.append(Some(&t::home_details()), Some(&format!("win.open-title(uint32 {id})"))); // i18n-skip
    if action == HomeAction::AddToPlanning {
        first.append(Some(&t::up_next_add_planning()), Some(&format!("win.plan(uint32 {id})"))); // i18n-skip
    }
    menu.append_section(None, &first);
    let second = gio::Menu::new();
    if mark {
        second.append(
            Some(&t::home_mark_watched(i64::from(episode))),
            Some(&format!("win.home-mark-watched({play})")),
        ); // i18n-skip
    }
    if kind == SectionKind::Continue {
        second.append(
            Some(&t::home_remove_continue()),
            Some(&format!("win.hide-continue(uint32 {id})")),
        ); // i18n-skip
    }
    if suggestion {
        second.append(
            Some(&t::up_next_not_interested()),
            Some(&format!("win.home-dismiss(uint32 {id})")),
        ); // i18n-skip
    }
    if second.n_items() > 0 {
        menu.append_section(None, &second);
    }
    menu
}

/// Placeholders at final size, each a plate where a card will land; This week's as its days.
fn placeholder_shelf(part: HomePart, first: bool) -> gtk::Widget {
    let outer = gtk::Box::new(gtk::Orientation::Vertical, 12);
    let heading = states::plate(140, 18, "bar");
    outer.append(&heading);
    // Clipped by a scroller that never scrolls, so the plates ask for no width of their own.
    let clip = gtk::Box::new(gtk::Orientation::Vertical, 0);
    let window = gtk::ScrolledWindow::builder()
        .hscrollbar_policy(gtk::PolicyType::External)
        .vscrollbar_policy(gtk::PolicyType::Never)
        .child(&clip)
        .build();
    window.set_propagate_natural_height(true);
    if part == HomePart::Week {
        let grid = gtk::Grid::new();
        grid.set_column_homogeneous(true);
        grid.set_column_spacing(GAP as u32);
        grid.add_css_class("home-week");
        for day in 0..DAYS {
            let column = gtk::Box::new(gtk::Orientation::Vertical, 12);
            column.set_size_request(DAY_WIDTH, -1);
            column.append(&states::plate(56, 10, "bar"));
            for _ in 0..2 {
                let entry = gtk::Box::new(gtk::Orientation::Horizontal, 8);
                entry.append(&states::plate(DAY_COVER, DAY_COVER * 3 / 2, ""));
                let text = gtk::Box::new(gtk::Orientation::Vertical, 8);
                text.append(&states::plate(40, 10, "bar"));
                text.append(&states::plate(64, 10, "bar"));
                entry.append(&text);
                column.append(&entry);
            }
            grid.attach(&column, day as i32, 0, 1, 1);
        }
        clip.append(&grid);
    } else {
        let row = gtk::Box::new(gtk::Orientation::Horizontal, GAP);
        let landscape = matches!(part, HomePart::Suggestions) && first;
        let (w, h) = if landscape {
            (CARD_WIDTH, CARD_WIDTH * 9 / 16)
        } else {
            (POSTER_WIDTH, POSTER_WIDTH * 3 / 2)
        };
        for _ in 0..8 {
            let card = gtk::Box::new(gtk::Orientation::Vertical, 8);
            card.append(&states::plate(w, h, ""));
            card.append(&states::plate(w * 7 / 10, 12, "bar"));
            card.append(&states::plate(w * 45 / 100, 10, "bar"));
            row.append(&card);
        }
        clip.append(&row);
    }
    outer.append(&window);
    outer.set_can_target(false);
    outer.update_property(&[gtk::accessible::Property::Label(&t::state_loading())]);
    outer.upcast()
}
