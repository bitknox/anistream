//! Home: a spotlight over shelves. The spotlight is one title with its main action in amber;
//! under it the shelves the engine chose, Continue first. Every shelf is a horizontal
//! scrolled window with paging buttons that fade in while the pointer is over it. The engine
//! sends Home whole and again as each part lands; each slot is rebuilt only when what it shows
//! changed, so a shelf the person has scrolled keeps its place.

use std::{
    cell::{Cell, RefCell},
    collections::HashMap,
    rc::Rc,
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
        HomeAction, HomeEnd, HomeItem, HomePart, HomeSection, HomeView, Playable, SectionKind,
        Spotlight, SpotlightCase,
    },
};
use gtk::{gio, glib};

use crate::{
    motion::{self, Fader},
    say, states, widgets,
    window::Ctx,
};

/// The column Home keeps to.
const COLUMN: i32 = 990;
/// The spotlight's art, 16:9.
const ART_WIDTH: i32 = 560;
/// A Continue card's width, 16:9.
const CARD_WIDTH: i32 = 272;
/// A poster card's width, 2:3.
const POSTER_WIDTH: i32 = 128;
const GAP: i32 = 16;
/// How long a wait is silent before placeholders show.
const SILENT: Duration = Duration::from_millis(600);
/// When the activity line joins the capsule.
const ACTIVITY: Duration = Duration::from_secs(4);

/// A slot to show: its key, what it shows as text, and how to build it.
type Wanted = (String, String, Box<dyn Fn() -> gtk::Widget>);

struct Slot {
    key: String,
    signature: String,
    widget: gtk::Widget,
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
    capsule_dot: gtk::Box,
    capsule_spin: gtk::Spinner,
    last: RefCell<Option<HomeView>>,
    /// Whether placeholders are on screen, and since when.
    placeholders: Cell<Option<Instant>>,
    silent_timer: RefCell<Option<glib::SourceId>>,
    activity_timer: RefCell<Option<glib::SourceId>>,
    activity: Cell<bool>,
    failed_told: Cell<bool>,
    probed: Cell<Option<AnilistId>>,
}

pub fn build(ctx: &Rc<Ctx>) -> adw::NavigationPage {
    let header = adw::HeaderBar::new();
    header.pack_start(&widgets::sidebar_button(&ctx.split));
    header.pack_start(&crate::window::tv_session_button(ctx));

    let capsule = gtk::Box::new(gtk::Orientation::Horizontal, 6);
    capsule.add_css_class("home-capsule");
    let capsule_dot = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    capsule_dot.add_css_class("state-dot");
    capsule_dot.set_valign(gtk::Align::Center);
    let capsule_spin = gtk::Spinner::new();
    capsule_spin.set_visible(false);
    let capsule_text = gtk::Label::new(None);
    capsule_text.add_css_class("caption");
    capsule_text.add_css_class("haze");
    capsule.append(&capsule_dot);
    capsule.append(&capsule_spin);
    capsule.append(&capsule_text);
    capsule.set_visible(false);
    header.pack_end(&capsule);

    let refresh = gtk::Button::from_icon_name("view-refresh-symbolic");
    refresh.set_tooltip_text(Some(&t::action_try_again()));
    refresh.set_action_name(Some("win.home-reload"));
    refresh.add_css_class("flat");
    header.pack_end(&refresh);

    let column = gtk::Box::new(gtk::Orientation::Vertical, 0);
    column.set_margin_top(12);
    column.set_margin_bottom(36);
    column.set_margin_start(12);
    column.set_margin_end(12);
    let clamp = adw::Clamp::builder().maximum_size(COLUMN).child(&column).build();
    let scroll = gtk::ScrolledWindow::builder()
        .hscrollbar_policy(gtk::PolicyType::Never)
        .vexpand(true)
        .child(&clamp)
        .build();

    let offline_empty = adw::StatusPage::builder()
        .icon_name("network-offline-symbolic")
        .title(t::home_offline_empty_title())
        .description(t::home_offline_empty_body())
        .build();
    let open = gtk::Button::with_label(&t::home_open_downloads());
    open.add_css_class("pill");
    open.set_halign(gtk::Align::Center);
    open.set_action_name(Some("win.go"));
    open.set_action_target_value(Some(&"downloads".to_variant()));
    offline_empty.set_child(Some(&open));

    let stack = gtk::Stack::new();
    stack.set_transition_type(gtk::StackTransitionType::Crossfade);
    stack.set_transition_duration(200);
    stack.add_named(&gtk::Box::new(gtk::Orientation::Vertical, 0), Some("blank"));
    stack.add_named(&scroll, Some("content"));
    stack.add_named(&offline_empty, Some("offline"));
    stack.set_visible_child_name("blank");

    let spot = gtk::Stack::new();
    spot.set_transition_type(gtk::StackTransitionType::Crossfade);
    spot.set_transition_duration(300);
    spot.set_hhomogeneous(false);
    spot.set_vhomogeneous(false);
    spot.set_interpolate_size(false);

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
        capsule_dot,
        capsule_spin,
        last: RefCell::new(None),
        placeholders: Cell::new(None),
        silent_timer: RefCell::new(None),
        activity_timer: RefCell::new(None),
        activity: Cell::new(false),
        failed_told: Cell::new(false),
        probed: Cell::new(None),
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

    let listener = ctx.hub.listen({
        let page = Rc::downgrade(&page);
        move |event| {
            if let (Some(page), Event::Home(view)) = (page.upgrade(), event) {
                page.apply(view);
            }
        }
    });
    let nav = widgets::content_page(&t::home_title(), &header, &page.stack);
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
        let before = self.last.replace(Some(view.clone()));
        let empty = view.spotlight.is_none() && view.sections.is_empty();
        self.watch_pending(view);
        self.capsule_state(view);
        self.tell_failure(view);
        if empty && view.pending.is_empty() && view.offline {
            self.stack.set_visible_child_name("offline");
            return;
        }
        if empty && !view.pending.is_empty() && self.placeholders.get().is_none() {
            // The first 600 ms are silent: the ground, then the shape of what comes.
            if self.stack.visible_child_name().as_deref() != Some("content") {
                self.stack.set_visible_child_name("blank");
            }
        } else {
            self.stack.set_visible_child_name("content");
        }
        self.rebuild(view, before.as_ref());
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
                        page.stack.set_visible_child_name("content");
                        page.redraw();
                    }
                });
                self.silent_timer.replace(Some(id));
            }
            if self.activity_timer.borrow().is_none() {
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
            self.rebuild(&view, Some(&view));
        }
    }

    /// The capsule in the header: synced, offline since a time, or loading suggestions.
    fn capsule_state(&self, view: &HomeView) {
        let signed_in = self.ctx.hub.known.borrow().anilist().is_some();
        self.capsule_spin.set_visible(false);
        self.capsule_spin.set_spinning(false);
        self.capsule_dot.set_visible(false);
        let text = if view.offline {
            let at = view
                .offline_since
                .and_then(|s| glib::DateTime::from_unix_local(s).ok())
                .and_then(|d| d.format("%H:%M").ok())
                .map(|s| s.to_string())
                .unwrap_or_default();
            Some(t::home_offline(&at))
        } else if self.activity.get() && !view.pending.is_empty() {
            self.capsule_spin.set_visible(true);
            self.capsule_spin.set_spinning(true);
            Some(t::home_loading())
        } else if signed_in && view.pending.is_empty() {
            self.capsule_dot.set_visible(true);
            Some(t::home_synced())
        } else {
            None
        };
        match text {
            Some(text) => {
                self.capsule_text.set_label(&text);
                self.capsule.set_visible(true);
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

    fn rebuild(self: &Rc<Self>, view: &HomeView, before: Option<&HomeView>) {
        let mut wanted: Vec<Wanted> = Vec::new();

        if let Some(spotlight) = &view.spotlight {
            let moved = before.is_none_or(|b| view.spotlight_moved(b));
            self.set_spotlight(spotlight, moved);
            wanted.push(("spotlight".into(), String::new(), {
                let spot = self.spot.clone();
                Box::new(move || spot.clone().upcast())
            }));
        } else {
            self.spot_signature.replace(String::new());
            self.spot_key.replace(None);
        }
        for section in &view.sections {
            let key = format!("{:?}", section.kind);
            let signature = format!("{section:?}");
            let page = Rc::clone(self);
            let section = section.clone();
            wanted.push((key, signature, Box::new(move || page.shelf(&section))));
        }
        if self.placeholders.get().is_some() {
            for (n, part) in view.pending.iter().enumerate() {
                let key = format!("ph-{part:?}");
                let part = *part;
                let first = n == 0 && view.spotlight.is_none() && view.sections.is_empty();
                wanted.push((
                    key,
                    String::new(),
                    Box::new(move || placeholder_shelf(part, first)),
                ));
            }
        }
        if let Some(end) = view.end.filter(|_| view.pending.is_empty()) {
            let page = Rc::clone(self);
            wanted.push((
                "end".into(),
                format!("{end:?}"),
                Box::new(move || page.closing(end)),
            ));
        }

        let had_placeholders = self.slots.borrow().iter().any(|s| s.key.starts_with("ph-"));
        let mut old: HashMap<String, Slot> =
            self.slots.take().into_iter().map(|s| (s.key.clone(), s)).collect();
        let mut slots = Vec::new();
        for (key, signature, make) in wanted {
            let keep =
                old.remove(&key).filter(|s| key == "spotlight" || s.signature == signature);
            let slot = match keep {
                Some(slot) => slot,
                None => {
                    let widget = make();
                    if had_placeholders && !key.starts_with("ph-") && key != "spotlight" {
                        motion::crossfade_in(&widget);
                    }
                    Slot { key, signature, widget }
                }
            };
            slots.push(slot);
        }
        // Detach what went, then lay the rest out in order.
        while let Some(child) = self.column.first_child() {
            self.column.remove(&child);
        }
        for (n, slot) in slots.iter().enumerate() {
            slot.widget.set_margin_top(if n == 0 { 0 } else { 36 });
            self.column.append(&slot.widget);
        }
        self.slots.replace(slots);
    }

    fn set_spotlight(self: &Rc<Self>, spotlight: &Spotlight, moved: bool) {
        let signature = format!("{spotlight:?}");
        if *self.spot_signature.borrow() == signature {
            return;
        }
        self.spot_signature.replace(signature);
        let key = (spotlight.entry.id, spotlight.case);
        let changed = self.spot_key.replace(Some(key)) != Some(key);
        if (moved || changed) && self.probed.replace(Some(key.0)) != Some(key.0) {
            self.ctx.hub.send(Command::ProbeTitle { id: key.0 });
        }
        let widget = self.spotlight(spotlight);
        let old = self.spot.first_child();
        self.spot.set_transition_type(if (moved || changed) && old.is_some() {
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
        info.set_hexpand(true);
        info.set_valign(gtk::Align::Center);

        let label = gtk::Label::new(Some(&say::msg(&s.label)));
        label.add_css_class("haze");
        label.add_css_class("home-label");
        label.set_xalign(0.0);
        label.set_margin_bottom(8);
        info.append(&label);

        let title = gtk::Label::new(Some(&s.entry.title));
        title.add_css_class("home-spot-title");
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
        more.add_css_class("home-sec");
        more.set_tooltip_text(Some(&t::home_more()));
        more.set_menu_model(Some(&card_menu(
            &s.entry,
            SectionKind::Starting,
            s.action,
            s.episode,
            s.playable,
            s.case != SpotlightCase::Continuing,
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
        let art = crate::fit::Fit::new(&frame, 16.0 / 9.0, ART_WIDTH, false, 0);
        art.set_halign(gtk::Align::End);
        art.set_valign(gtk::Align::Center);
        art.set_hexpand(false);
        art.update_property(&[gtk::accessible::Property::Label(&s.entry.title)]);

        let spot = gtk::Box::new(gtk::Orientation::Horizontal, 36);
        spot.add_css_class("home-spot");
        spot.append(&info);
        spot.append(&art);
        spot.upcast()
    }

    /// A shelf: heading, cards, paging buttons while the pointer is over it.
    fn shelf(self: &Rc<Self>, section: &HomeSection) -> gtk::Widget {
        let outer = gtk::Box::new(gtk::Orientation::Vertical, 12);

        let head = gtk::Box::new(gtk::Orientation::Horizontal, 10);
        let title = gtk::Label::new(Some(&say::msg(&section.title)));
        title.add_css_class("home-heading");
        title.set_xalign(0.0);
        title.set_accessible_role(gtk::AccessibleRole::Heading);
        head.append(&title);
        if section.kind == SectionKind::Continue && section.total > 0 {
            head.append(&line(&t::home_continue_count(i64::from(section.total)), "haze", 0));
        }
        let spacer = gtk::Box::new(gtk::Orientation::Horizontal, 0);
        spacer.set_hexpand(true);
        head.append(&spacer);
        if let Some((text, dest)) = section.view_all.as_ref().map(|l| self.link(l)) {
            let all = gtk::Button::with_label(&text);
            all.add_css_class("flat");
            all.add_css_class("home-all");
            all.set_action_name(Some("win.go"));
            all.set_action_target_value(Some(&dest.to_variant()));
            head.append(&all);
        }
        outer.append(&head);

        let cards = gtk::Box::new(gtk::Orientation::Horizontal, GAP);
        for item in &section.items {
            cards.append(&self.card(section.kind, item));
        }
        let scroll = gtk::ScrolledWindow::builder()
            .hscrollbar_policy(gtk::PolicyType::External)
            .vscrollbar_policy(gtk::PolicyType::Never)
            .child(&cards)
            .build();
        scroll.set_propagate_natural_height(true);
        let layers = gtk::Overlay::new();
        layers.set_child(Some(&scroll));
        let art_height = if section.kind == SectionKind::Continue {
            CARD_WIDTH * 9 / 16
        } else {
            POSTER_WIDTH * 3 / 2
        };
        let prev =
            arrow("go-previous-symbolic", &t::home_previous(), gtk::Align::Start, art_height);
        let next = arrow("go-next-symbolic", &t::home_next(), gtk::Align::End, art_height);
        layers.add_overlay(&prev);
        layers.add_overlay(&next);
        outer.append(&layers);

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
        let motion_ctl = gtk::EventControllerMotion::new();
        motion_ctl.connect_enter({
            let (over, refresh) = (over.clone(), refresh.clone());
            move |_, _, _| {
                over.set(true);
                refresh();
            }
        });
        motion_ctl.connect_leave({
            let (over, refresh) = (over.clone(), refresh.clone());
            move |_| {
                over.set(false);
                refresh();
            }
        });
        layers.add_controller(motion_ctl);
        prev.connect_clicked({
            let scroll = scroll.clone();
            move |_| page_by(&scroll, -1.0)
        });
        next.connect_clicked({
            let scroll = scroll.clone();
            move |_| page_by(&scroll, 1.0)
        });
        widgets::keep(&outer, (fade, refresh, over));
        outer.upcast()
    }

    fn link(&self, link: &anistream_model::home::HomeLink) -> (String, &'static str) {
        use anistream_model::home::HomeLink as L;
        match link {
            L::Watching => (t::home_view_all(), "library"),
            L::SeasonalView => (t::home_seasonal(), "seasonal"),
            L::CalendarView => (t::home_full_calendar(), "calendar"),
            L::Browse { .. } => (t::home_search(), "search"),
        }
    }

    fn card(self: &Rc<Self>, kind: SectionKind, item: &HomeItem) -> gtk::Widget {
        let continuing = kind == SectionKind::Continue;
        let width = if continuing { CARD_WIDTH } else { POSTER_WIDTH };
        let inner = gtk::Box::new(gtk::Orientation::Vertical, 0);
        inner.set_size_request(width, -1);
        inner.set_halign(gtk::Align::Start);

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

        let title = gtk::Label::new(Some(&item.entry.title));
        title.add_css_class(if continuing { "home-card-title" } else { "home-poster-title" });
        title.set_xalign(0.0);
        title.set_ellipsize(gtk::pango::EllipsizeMode::End);
        title.set_max_width_chars(width / 7);
        title.set_margin_top(8);
        inner.append(&title);

        let text = item
            .line
            .as_ref()
            .map(say::msg)
            .or_else(|| (item.playable == Playable::NotFound).then(t::home_not_found))
            .unwrap_or_else(|| say::meta_line(&item.entry));
        let sub = gtk::Box::new(gtk::Orientation::Horizontal, 6);
        if item.new {
            let dot = gtk::Box::new(gtk::Orientation::Horizontal, 0);
            dot.add_css_class("home-new");
            dot.set_valign(gtk::Align::Center);
            sub.append(&dot);
        }
        let sub_text = gtk::Label::new(Some(&text));
        sub_text.add_css_class("haze");
        sub_text.add_css_class("home-sub");
        sub_text.set_xalign(0.0);
        sub_text.set_ellipsize(gtk::pango::EllipsizeMode::End);
        sub_text.set_max_width_chars(width / 7);
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
        widgets::context_menu(
            &button,
            &card_menu(&item.entry, kind, item.action, episode, item.playable, suggestion),
        );
        button.upcast()
    }

    fn closing(&self, end: HomeEnd) -> gtk::Widget {
        let column = gtk::Box::new(gtk::Orientation::Vertical, 8);
        column.set_halign(gtk::Align::Center);
        let (text, button) = match end {
            HomeEnd::Everything => (t::home_end(), None),
            HomeEnd::SignIn => (
                t::home_end_signed_out(),
                Some((t::home_sign_in(), "win.settings-pane", "accounts")),
            ),
            HomeEnd::NoSources => {
                (t::home_end_no_sources(), Some((t::nav_plugin_stores(), "win.go", "stores")))
            }
        };
        let l = gtk::Label::new(Some(&text));
        l.add_css_class("haze");
        l.set_wrap(true);
        l.set_justify(gtk::Justification::Center);
        column.append(&l);
        if let Some((label, action, target)) = button {
            let b = gtk::Button::with_label(&label);
            b.add_css_class("flat");
            b.set_halign(gtk::Align::Center);
            b.set_action_name(Some(action));
            b.set_action_target_value(Some(&target.to_variant()));
            column.append(&b);
        }
        column.set_margin_top(8);
        column.upcast()
    }

    /// Not interested: the poster fades, the neighbours close the gap when Home answers, and a
    /// notice offers Undo.
    fn dismiss(self: &Rc<Self>, id: AnilistId) {
        let entry = self.ctx.hub.entry(id);
        let title = entry.as_ref().map_or_else(String::new, |e| e.title.clone());
        let until_airs = entry.as_ref().is_some_and(|e| {
            e.airing_in.is_some_and(|s| s > 0) && e.airing_episode.is_none_or(|n| n <= 1)
        });
        self.fade_cards_of(id);
        self.ctx.hub.send(Command::DismissUpNext { id });
        let text = if until_airs {
            t::up_next_hidden_until_airs(&title)
        } else {
            t::up_next_hidden(&title)
        };
        self.ctx.toast_to(&text, &t::action_undo(), "win.restore-up-next", &id.to_string());
    }

    fn fade_cards_of(&self, id: AnilistId) {
        let name = "win.open-title";
        let target = id.get().to_variant();
        let mut stack = vec![self.column.clone().upcast::<gtk::Widget>()];
        while let Some(w) = stack.pop() {
            if let Ok(button) = w.clone().downcast::<gtk::Button>()
                && button.action_name().as_deref() == Some(name)
                && button.action_target_value().as_ref() == Some(&target)
                && button.has_css_class("home-card")
            {
                motion::fade_out(&button);
            }
            let mut child = w.first_child();
            while let Some(c) = child {
                child = c.next_sibling();
                stack.push(c);
            }
        }
    }
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

/// A paging button. Hidden from Tab and screen readers: the scroll bar and the arrow keys do
/// the same work.
fn arrow(icon: &str, tooltip: &str, side: gtk::Align, art_height: i32) -> gtk::Button {
    let b = gtk::Button::from_icon_name(icon);
    b.add_css_class("circular");
    b.add_css_class("osd");
    b.add_css_class("home-arrow");
    b.set_halign(side);
    b.set_valign(gtk::Align::Start);
    b.set_margin_top(art_height / 2 - 18);
    b.set_margin_start(6);
    b.set_margin_end(6);
    b.set_size_request(36, 36);
    b.set_focusable(false);
    b.set_can_focus(false);
    b.set_tooltip_text(Some(tooltip));
    b.set_visible(false);
    b.update_property(&[gtk::accessible::Property::Label(tooltip)]);
    b
}

/// Scroll a shelf by one page, over 300 ms.
fn page_by(host: &gtk::ScrolledWindow, direction: f64) {
    let adj = host.hadjustment();
    let step = (adj.page_size() - f64::from(GAP)).max(1.0);
    let to = (adj.value() + direction * step)
        .clamp(adj.lower(), (adj.upper() - adj.page_size()).max(adj.lower()));
    let target = adw::CallbackAnimationTarget::new({
        let adj = adj.clone();
        move |v| adj.set_value(v)
    });
    let animation = adw::TimedAnimation::new(host, adj.value(), to, 300, target);
    animation.set_easing(adw::Easing::EaseOutCubic);
    animation.play();
}

/// The menu on a card and under the spotlight's more button.
fn card_menu(
    entry: &Entry,
    kind: SectionKind,
    action: HomeAction,
    episode: u32,
    playable: Playable,
    suggestion: bool,
) -> gio::Menu {
    let id = entry.id.get();
    let menu = gio::Menu::new();
    let first = gio::Menu::new();
    if playable != Playable::NotFound {
        let play = format!("(uint32 {id}, '{episode}')"); // i18n-skip
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

/// Placeholders at final size, each a plate where a card will land.
fn placeholder_shelf(part: HomePart, first: bool) -> gtk::Widget {
    let outer = gtk::Box::new(gtk::Orientation::Vertical, 12);
    let heading = states::plate(140, 18, "bar");
    outer.append(&heading);
    let row = gtk::Box::new(gtk::Orientation::Horizontal, GAP);
    let landscape = matches!(part, HomePart::Suggestions) && first;
    let (w, h) = if landscape {
        (CARD_WIDTH, CARD_WIDTH * 9 / 16)
    } else {
        (POSTER_WIDTH, POSTER_WIDTH * 3 / 2)
    };
    for _ in 0..6 {
        let card = gtk::Box::new(gtk::Orientation::Vertical, 8);
        card.append(&states::plate(w, h, ""));
        card.append(&states::plate(w * 7 / 10, 12, "bar"));
        card.append(&states::plate(w * 45 / 100, 10, "bar"));
        row.append(&card);
    }
    let clip = gtk::Box::new(gtk::Orientation::Vertical, 0);
    clip.set_overflow(gtk::Overflow::Hidden);
    clip.append(&row);
    outer.append(&clip);
    outer.set_can_target(false);
    outer.upcast()
}
