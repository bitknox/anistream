//! The five states every screen has: loading, content, empty, offline and failed, in one stack.
//!
//! Waiting is silent for 600 ms: the screen keeps what it showed, or the ground on a first
//! visit. Past that, static placeholders stand where the result will land, in its shape, and
//! past 4 s the job's activity line shows above them. Placeholders stay at least 0.3 s and give
//! way in one crossfade on the standard token. A load that fails says why, with Try Again; one
//! that fails, or has nothing, while the device is offline says so instead.

use std::{
    cell::{Cell, RefCell},
    rc::Rc,
    time::{Duration, Instant},
};

use adw::prelude::*;
use anistream_model::event::Event;
use gtk::glib;

use anistream_i18n::t;

use crate::{hub::Hub, motion};

/// How long a wait is silent.
const SILENT: Duration = Duration::from_millis(600);
/// When the activity line joins the placeholders.
const ACTIVITY: Duration = Duration::from_secs(4);
/// The least time placeholders stay once shown, so they never flash.
const AT_LEAST: Duration = Duration::from_millis(300);

/// What the placeholders look like: the shape of what lands there.
#[derive(Debug, Clone, Copy)]
pub enum Shape {
    /// A poster grid: covers with a title and a caption line under each.
    Grid,
    /// `count` rows, each with art `width` by `height` and two lines of text.
    Rows { count: usize, width: i32, height: i32 },
    /// The calendar: seven columns of banners on a wide window, day sections of rows on a
    /// narrower one, as `States::agenda` picks.
    Week,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Phase {
    /// The ground, or what was there before, inside the first 600 ms.
    Silent,
    /// Placeholders on screen, since then.
    Placeholders,
    Content,
    Empty,
    Failed,
    Offline,
}

/// A screen's states, shown in one stack.
pub struct States {
    pub widget: gtk::Stack,
    hub: Rc<Hub>,
    activity: gtk::Label,
    failed: adw::StatusPage,
    offline: adw::StatusPage,
    phase: Cell<Phase>,
    since: Cell<Option<Instant>>,
    timers: RefCell<Vec<glib::SourceId>>,
    pending: RefCell<Option<glib::SourceId>>,
    retry: RefCell<Option<Rc<dyn Fn()>>>,
    listening: Cell<u64>,
    /// The calendar's placeholders, "week" or "agenda".
    shapes: Option<gtk::Stack>,
}

impl States {
    /// `content` and `empty` as the screen draws them, the placeholders in `shape`, and the
    /// failed state headed `failed`: "Couldn't Load This Season".
    pub fn new(
        hub: &Rc<Hub>,
        content: &impl IsA<gtk::Widget>,
        shape: Shape,
        empty: &impl IsA<gtk::Widget>,
        failed: &str,
    ) -> Rc<Self> {
        let widget = gtk::Stack::new();
        widget.set_transition_type(gtk::StackTransitionType::None);
        widget.set_vexpand(true);
        widget.add_named(&gtk::Box::new(gtk::Orientation::Vertical, 0), Some("silent"));

        let activity = gtk::Label::new(None);
        activity.add_css_class("haze");
        activity.add_css_class("caption");
        activity.set_xalign(0.0);
        activity.set_opacity(0.0);
        let held = gtk::Box::new(gtk::Orientation::Vertical, 8);
        held.set_can_target(false);
        held.update_property(&[gtk::accessible::Property::Label(&t::state_loading())]);
        held.append(&activity);
        let drawn = placeholders(shape);
        held.append(&drawn);
        match shape {
            Shape::Grid => {
                activity.set_margin_start(32);
                held.set_margin_top(12);
            }
            Shape::Rows { .. } => {
                held.set_margin_start(16);
                held.set_margin_end(16);
                held.set_margin_top(16);
            }
            Shape::Week => held.set_margin_top(8),
        }
        // Clipped to the space the screen has: the placeholders never ask for more height than
        // the window, however many rows a narrow one wraps them into.
        let clip = gtk::ScrolledWindow::builder()
            .hscrollbar_policy(gtk::PolicyType::Never)
            .vscrollbar_policy(gtk::PolicyType::External)
            .child(&held)
            .build();
        clip.set_can_target(false);
        widget.add_named(&clip, Some("placeholders"));
        widget.add_named(content, Some("content"));
        widget.add_named(empty, Some("empty"));

        let failed = adw::StatusPage::builder()
            .title(failed)
            .icon_name("dialog-warning-symbolic")
            .build();
        failed.add_css_class("compact");
        failed.add_css_class("failed");
        let again = gtk::Button::with_label(&t::action_try_again());
        again.add_css_class("pill");
        again.set_halign(gtk::Align::Center);
        failed.set_child(Some(&again));
        widget.add_named(&failed, Some("failed"));

        let show_downloads = gtk::Button::with_label(&t::state_offline_show_downloads());
        show_downloads.add_css_class("pill");
        show_downloads.set_halign(gtk::Align::Center);
        show_downloads.set_action_name(Some("win.go"));
        show_downloads.set_action_target_value(Some(&"downloads".to_variant()));
        let offline = adw::StatusPage::builder()
            .title(t::state_offline_title())
            .description(t::state_offline_detail())
            .child(&show_downloads)
            .build();
        offline.add_css_class("compact");
        widget.add_named(&offline, Some("offline"));

        let states = Rc::new(Self {
            widget,
            hub: Rc::clone(hub),
            activity,
            failed,
            offline,
            phase: Cell::new(Phase::Silent),
            since: Cell::new(None),
            timers: RefCell::default(),
            pending: RefCell::default(),
            retry: RefCell::default(),
            listening: Cell::new(0),
            shapes: drawn.downcast::<gtk::Stack>().ok(),
        });
        again.connect_clicked({
            let weak = Rc::downgrade(&states);
            move |_| {
                let Some(states) = weak.upgrade() else { return };
                let retry = states.retry.borrow().clone();
                if let Some(retry) = retry {
                    retry();
                }
            }
        });
        let weak = Rc::downgrade(&states);
        let id = hub.listen(move |event| {
            if let (Some(states), Event::Connectivity { offline }) = (weak.upgrade(), event) {
                states.connectivity(*offline);
            }
        });
        states.listening.set(id);
        states.widget.connect_destroy({
            let weak = Rc::downgrade(&states);
            move |_| {
                if let Some(states) = weak.upgrade() {
                    states.hub.forget(states.listening.get());
                    states.cancel();
                }
            }
        });
        states
    }

    /// What the offline state says on this screen, when it is not the usual words.
    pub fn offline_words(&self, title: &str, line: &str) {
        self.offline.set_title(title);
        self.offline.set_description(Some(line));
    }

    /// What Try Again does: ask for the screen again.
    pub fn on_retry(&self, retry: impl Fn() + 'static) {
        self.retry.replace(Some(Rc::new(retry)));
    }

    /// Whether what shows is the screen's content or its empty state.
    pub fn has_answer(&self) -> bool {
        matches!(self.phase.get(), Phase::Content | Phase::Empty)
    }

    /// A load started, said by its job's activity ("loading this season"): silent for 600 ms,
    /// then placeholders, then the activity line.
    pub fn loading(self: &Rc<Self>, activity: Option<String>) {
        let label = activity.unwrap_or_default();
        // Already waiting: the wait goes on, with the newer words.
        let waiting = match self.phase.get() {
            Phase::Placeholders => true,
            Phase::Silent => !self.timers.borrow().is_empty(),
            _ => false,
        };
        if waiting {
            if !label.is_empty() {
                self.activity.set_label(&label);
            }
            return;
        }
        self.cancel();
        if self.hub.known.borrow().offline && !self.has_answer() {
            self.go(Phase::Offline);
            return;
        }
        self.phase.set(Phase::Silent);
        self.activity.set_label(&label);
        self.activity.set_opacity(0.0);
        let weak = Rc::downgrade(self);
        let show = glib::timeout_add_local_once(SILENT, move || {
            let Some(states) = weak.upgrade() else { return };
            // This timer has fired: forgotten, so a cancel does not remove it again.
            states.timers.borrow_mut().clear();
            if states.phase.get() != Phase::Silent {
                return;
            }
            states.phase.set(Phase::Placeholders);
            states.since.set(Some(Instant::now()));
            states.widget.set_visible_child_name("placeholders");
            if let Some(child) = states.widget.visible_child() {
                motion::crossfade_in(&child);
            }
            let weak = Rc::downgrade(&states);
            let line = glib::timeout_add_local_once(ACTIVITY - SILENT, move || {
                let Some(states) = weak.upgrade() else { return };
                states.timers.borrow_mut().clear();
                if states.phase.get() == Phase::Placeholders
                    && !states.activity.label().is_empty()
                {
                    motion::fade_in(&states.activity);
                }
            });
            states.timers.borrow_mut().push(line);
        });
        self.timers.borrow_mut().push(show);
    }

    /// The calendar's placeholders as an agenda, for a window too narrow for the week.
    pub fn agenda(&self, agenda: bool) {
        if let Some(shapes) = &self.shapes {
            shapes.set_visible_child_name(if agenda { "agenda" } else { "week" });
        }
    }

    pub fn content(self: &Rc<Self>) {
        self.answer(Phase::Content);
    }

    pub fn empty(self: &Rc<Self>) {
        self.answer(Phase::Empty);
    }

    /// The load failed, `why` in the engine's words, which are for the log: the page says it in
    /// the catalogue's, or shows the offline state while there is no network.
    pub fn failed(self: &Rc<Self>, why: &str) {
        tracing::debug!(%why, "a load failed");
        if self.hub.known.borrow().offline {
            self.answer(Phase::Offline);
            return;
        }
        self.failed.set_description(Some(&t::error_generic()));
        self.answer(Phase::Failed);
    }

    fn connectivity(self: &Rc<Self>, offline: bool) {
        match (offline, self.phase.get()) {
            (true, Phase::Silent | Phase::Placeholders | Phase::Failed) => {
                self.cancel();
                self.go(Phase::Offline);
            }
            // Back online, the engine asks again for what failed: wait for it.
            (false, Phase::Offline) => self.loading(None),
            _ => {}
        }
    }

    /// Show `phase` once the placeholders have stood their least time.
    fn answer(self: &Rc<Self>, phase: Phase) {
        self.cancel();
        let left = match (self.phase.get(), self.since.get()) {
            (Phase::Placeholders, Some(since)) => AT_LEAST.saturating_sub(since.elapsed()),
            _ => Duration::ZERO,
        };
        if left.is_zero() {
            self.go(phase);
            return;
        }
        let weak = Rc::downgrade(self);
        let pending = glib::timeout_add_local_once(left, move || {
            if let Some(states) = weak.upgrade() {
                states.pending.borrow_mut().take();
                show(&states.widget, phase, true);
            }
        });
        // The phase is the answer's from now, so a later answer replaces this one.
        self.phase.set(phase);
        self.pending.replace(Some(pending));
    }

    fn go(&self, phase: Phase) {
        let from = self.phase.replace(phase);
        show(&self.widget, phase, from == Phase::Placeholders);
    }

    fn cancel(&self) {
        for timer in self.timers.borrow_mut().drain(..) {
            timer.remove();
        }
        if let Some(pending) = self.pending.borrow_mut().take() {
            pending.remove();
        }
    }
}

fn show(stack: &gtk::Stack, phase: Phase, crossfade: bool) {
    let name = match phase {
        Phase::Silent => "silent",
        Phase::Placeholders => "placeholders",
        Phase::Content => "content",
        Phase::Empty => "empty",
        Phase::Failed => "failed",
        Phase::Offline => "offline",
    };
    if stack.visible_child_name().as_deref() == Some(name) {
        return;
    }
    stack.set_visible_child_name(name);
    if crossfade && let Some(child) = stack.visible_child() {
        motion::crossfade_in(&child);
    }
}

/// A small, frequent empty: a filtered list, a list of the library, a quiet week. No Nagare.
pub fn plain(title: &str, line: &str) -> adw::StatusPage {
    let page = adw::StatusPage::builder().title(title).description(line).build();
    page.add_css_class("compact");
    page
}

/// A raised plate where art or a line of text will land.
fn plate(width: i32, height: i32, class: &str) -> gtk::Box {
    let plate = gtk::Box::new(gtk::Orientation::Vertical, 0);
    plate.add_css_class("plate");
    if !class.is_empty() {
        plate.add_css_class(class);
    }
    plate.set_size_request(width, height);
    plate.set_halign(gtk::Align::Start);
    plate
}

fn placeholders(shape: Shape) -> gtk::Widget {
    match shape {
        Shape::Grid => {
            let grid = gtk::FlowBox::builder()
                .selection_mode(gtk::SelectionMode::None)
                .homogeneous(true)
                .min_children_per_line(2)
                .max_children_per_line(12)
                .column_spacing(16)
                .row_spacing(16)
                .valign(gtk::Align::Start)
                .build();
            grid.set_margin_start(32);
            grid.set_margin_end(32);
            grid.set_can_focus(false);
            for _ in 0..12 {
                let poster = gtk::Box::new(gtk::Orientation::Vertical, 8);
                poster.append(&plate(140, 210, ""));
                poster.append(&plate(110, 14, "bar"));
                poster.append(&plate(80, 10, "bar"));
                grid.append(&poster);
            }
            let clip = gtk::Box::new(gtk::Orientation::Vertical, 0);
            clip.set_overflow(gtk::Overflow::Hidden);
            clip.set_vexpand(true);
            clip.append(&grid);
            clip.upcast()
        }
        Shape::Rows { count, width, height } => {
            let rows = gtk::Box::new(gtk::Orientation::Vertical, 12);
            for _ in 0..count {
                let row = gtk::Box::new(gtk::Orientation::Horizontal, 12);
                row.append(&plate(width, height, ""));
                let text = gtk::Box::new(gtk::Orientation::Vertical, 8);
                text.set_valign(gtk::Align::Center);
                text.append(&plate(160, 14, "bar"));
                text.append(&plate(100, 10, "bar"));
                row.append(&text);
                rows.append(&row);
            }
            rows.upcast()
        }
        Shape::Week => {
            let week = gtk::Box::new(gtk::Orientation::Horizontal, 4);
            week.set_homogeneous(true);
            week.set_margin_start(12);
            week.set_margin_end(12);
            for _ in 0..7 {
                let day = gtk::Box::new(gtk::Orientation::Vertical, 10);
                day.set_margin_start(6);
                day.set_margin_end(6);
                day.set_margin_top(8);
                day.append(&plate(28, 10, "bar"));
                day.append(&plate(20, 18, "bar"));
                for _ in 0..2 {
                    let banner = plate(-1, 64, "");
                    banner.set_halign(gtk::Align::Fill);
                    day.append(&banner);
                    // Wide as the column allows, so seven of them fit a narrow window.
                    let line = plate(-1, 12, "bar");
                    line.set_halign(gtk::Align::Fill);
                    line.set_margin_end(16);
                    day.append(&line);
                }
                week.append(&day);
            }
            let agenda = gtk::Box::new(gtk::Orientation::Vertical, 18);
            agenda.set_margin_start(18);
            agenda.set_margin_end(18);
            agenda.set_margin_top(6);
            for rows in [2, 1] {
                let section = gtk::Box::new(gtk::Orientation::Vertical, 10);
                section.append(&plate(180, 18, "bar"));
                for _ in 0..rows {
                    let row = gtk::Box::new(gtk::Orientation::Horizontal, 12);
                    row.append(&plate(80, 45, ""));
                    let text = gtk::Box::new(gtk::Orientation::Vertical, 8);
                    text.set_valign(gtk::Align::Center);
                    text.append(&plate(160, 14, "bar"));
                    text.append(&plate(90, 10, "bar"));
                    row.append(&text);
                    section.append(&row);
                }
                agenda.append(&section);
            }
            let shapes = gtk::Stack::new();
            shapes.set_hhomogeneous(false);
            shapes.set_vhomogeneous(false);
            shapes.add_named(&week, Some("week"));
            shapes.add_named(&agenda, Some("agenda"));
            shapes.upcast()
        }
    }
}
