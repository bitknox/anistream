//! The TV sheet, as the Mac's: the picker that finds TVs and pairs with one, the six digits a TV
//! shows, and the remote for the TV driven. One dialog, which a narrow window shows as a bottom
//! sheet; its face follows the model. Every word about the link and what went wrong is the
//! engine's.

use std::{
    cell::{Cell, RefCell},
    rc::Rc,
    time::{Duration, Instant},
};

use adw::prelude::*;
use anistream_engine::remote::{
    protocol::{
        command::{RemoteCommand, Track},
        state::TrackInfo,
    },
    sender::{LinkPhase, NowCard, TvRow, TvRowKind},
};
use anistream_model::hosted::TrackKind;
use gtk::{gdk, gio, glib};

use super::{Sheet as Shown, Tv};
use crate::{
    widgets,
    window::{Ctx, Size},
    words,
};

/// The picker says nothing for this long while no TV is listed, as most TVs answer by then.
const QUIET_FOR: Duration = Duration::from_millis(600);
/// And after this long with none, what to check. The browse goes on.
const GIVE_UP_AFTER: Duration = Duration::from_secs(10);
/// A seek dragged on the line is sent once the drag rests this long.
const SEEK_REST: Duration = Duration::from_millis(250);
/// Where a seek let go is held on the line until the TV says where it went, or this long.
const SEEK_HOLD: Duration = Duration::from_secs(3);
/// TV Volume's step, each press.
const VOLUME_STEP: f64 = 5.0;

/// Open the sheet whenever the model asks for it, and close it when the model lets it go.
pub fn attach(ctx: &Rc<Ctx>) {
    let showing: Rc<RefCell<Option<Rc<Dialog>>>> = Rc::default();
    let weak = Rc::downgrade(ctx);
    ctx.tv.listen(move || {
        let Some(ctx) = weak.upgrade() else { return };
        let up = showing.borrow().clone();
        match (ctx.tv.sheet(), up) {
            (Some(_), Some(dialog)) => dialog.update(),
            (Some(_), None) => {
                let dialog = Dialog::new(&ctx, &showing);
                showing.replace(Some(Rc::clone(&dialog)));
                #[cfg(feature = "fixtures")]
                SHOWN.with(|s| s.replace(Rc::downgrade(&dialog)));
                dialog.update();
                dialog.dialog.present(Some(&ctx.window));
            }
            (None, Some(dialog)) => {
                showing.replace(None);
                dialog.dialog.force_close();
            }
            (None, None) => {}
        }
    });
    // The window back at the front: a TV lost meanwhile is tried at once.
    ctx.window.connect_is_active_notify({
        let tv = Rc::downgrade(&ctx.tv);
        move |window| {
            if window.is_active()
                && let Some(tv) = tv.upgrade()
            {
                tv.retry_now();
            }
        }
    });
}

#[cfg(feature = "fixtures")]
thread_local! {
    /// The sheet that is up, for a shot to type into.
    static SHOWN: RefCell<std::rc::Weak<Dialog>> = const { RefCell::new(std::rc::Weak::new()) };
}

/// For a shot: the picker's address field, open, with `text` typed.
#[cfg(feature = "fixtures")]
pub fn shot_address(text: &str) {
    if let Some(dialog) = SHOWN.with(|s| s.borrow().upgrade()) {
        dialog.picker.reveal_address();
        dialog.picker.address.set_text(text);
        dialog.picker.address.set_position(-1);
    }
}

/// For a shot: `digits` typed into the pairing code.
#[cfg(feature = "fixtures")]
pub fn shot_code(digits: &str) {
    if let Some(dialog) = SHOWN.with(|s| s.borrow().upgrade()) {
        dialog.pairing.code.focus();
        dialog.pairing.code.text.set_text(digits);
        dialog.pairing.code.text.set_position(-1);
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Face {
    Picker,
    Pairing,
    Remote,
}

struct Dialog {
    tv: Rc<Tv>,
    ctx: Rc<Ctx>,
    dialog: adw::Dialog,
    stack: gtk::Stack,
    face: Cell<Option<Face>>,
    browsing: Cell<bool>,
    picker: Picker,
    pairing: Pairing,
    remote: Remote,
    tick: RefCell<Option<glib::SourceId>>,
}

impl Dialog {
    fn new(ctx: &Rc<Ctx>, showing: &Rc<RefCell<Option<Rc<Self>>>>) -> Rc<Self> {
        let picker = Picker::new();
        let pairing = Pairing::new();
        let remote = Remote::new();
        let stack = gtk::Stack::builder()
            .transition_type(gtk::StackTransitionType::Crossfade)
            .transition_duration(150)
            .hhomogeneous(false)
            .vhomogeneous(false)
            .interpolate_size(true)
            .build();
        stack.add_named(&picker.view, Some("picker"));
        stack.add_named(&pairing.view, Some("pairing"));
        stack.add_named(&remote.view, Some("remote"));
        let dialog = adw::Dialog::builder()
            .name("tv")
            .title("Play on TV")
            .follows_content_size(true)
            .child(&stack)
            .build();
        if ctx.size() != Size::Narrow {
            stack.set_size_request(440, -1);
        }
        let me = Rc::new(Self {
            tv: Rc::clone(&ctx.tv),
            ctx: Rc::clone(ctx),
            dialog,
            stack,
            face: Cell::new(None),
            browsing: Cell::new(false),
            picker,
            pairing,
            remote,
            tick: RefCell::default(),
        });
        me.wire();
        // Closed by Done, Escape or a swipe: the model lets the sheet go at once, so a sheet
        // asked for while this one animates away is a new one, not this one going.
        me.dialog.set_can_close(false);
        me.dialog.connect_close_attempt({
            let tv = Rc::downgrade(&me.tv);
            move |dialog| match tv.upgrade() {
                Some(tv) if tv.sheet().is_some() => tv.sheet_closed(),
                _ => dialog.force_close(),
            }
        });
        me.dialog.connect_closed({
            let me = Rc::downgrade(&me);
            let showing = Rc::downgrade(showing);
            move |_| {
                let Some(me) = me.upgrade() else { return };
                if let Some(id) = me.tick.take() {
                    id.remove();
                }
                me.browse(false);
                // Closed without the model's word, as by force: let the sheet go, unless a
                // newer one is up by now.
                let Some(showing) = showing.upgrade() else { return };
                let mine = showing.borrow().as_ref().is_some_and(|up| Rc::ptr_eq(up, &me));
                if mine {
                    showing.replace(None);
                    me.tv.sheet_closed();
                }
            }
        });
        // Twice a second: the searching row's stage, and the TV's playhead on the seek line.
        let tick = glib::timeout_add_local(Duration::from_millis(500), {
            let me = Rc::downgrade(&me);
            move || {
                let Some(me) = me.upgrade() else { return glib::ControlFlow::Break };
                match me.face.get() {
                    Some(Face::Picker) => me.picker.show_searching(&me.tv),
                    Some(Face::Remote) => me.remote.show_playhead(&me.tv),
                    _ => {}
                }
                glib::ControlFlow::Continue
            }
        });
        me.tick.replace(Some(tick));
        me
    }

    fn browse(&self, on: bool) {
        if self.browsing.replace(on) == on {
            return;
        }
        if on {
            self.picker.since.set(Instant::now());
            self.tv.start_browsing();
        } else {
            self.tv.stop_browsing();
        }
    }

    /// Draw what the model holds, on the face it calls for.
    fn update(self: &Rc<Self>) {
        let tv = &self.tv;
        let face = if tv.pairing().is_some() {
            Face::Pairing
        } else if tv.sheet() == Some(Shown::Remote) {
            Face::Remote
        } else {
            Face::Picker
        };
        let came = self.face.replace(Some(face)) != Some(face);
        // Pairing goes on browsing: a TV chosen from the list is found by what the browse
        // saw, and Back returns to a list still up to date.
        self.browse(face != Face::Remote);
        match face {
            Face::Picker => self.picker.update(tv, came),
            Face::Pairing => self.pairing.update(tv, came),
            Face::Remote => self.remote.update(&self.ctx, came),
        }
        let (name, title) = match face {
            Face::Picker => ("picker", self.picker.title.title()),
            Face::Pairing => ("pairing", self.pairing.title.title()),
            Face::Remote => ("remote", self.remote.title.title()),
        };
        self.dialog.set_title(&title);
        if came {
            self.stack.set_visible_child_name(name);
            let default = match face {
                Face::Picker | Face::Remote => None,
                Face::Pairing => Some(&self.pairing.pair),
            };
            self.dialog.set_default_widget(default);
            match face {
                Face::Pairing => {
                    self.pairing.code.focus();
                }
                Face::Picker => {
                    self.picker.done.grab_focus();
                }
                Face::Remote => {
                    self.remote.done.grab_focus();
                }
            }
        }
    }

    fn wire(self: &Rc<Self>) {
        let weak = Rc::downgrade(self);
        let on = move |f: fn(&Dialog)| {
            let weak = weak.clone();
            move || {
                if let Some(me) = weak.upgrade() {
                    f(&me);
                }
            }
        };
        let close = on(|me| {
            me.dialog.close();
        });
        let close_too = close.clone();
        self.picker.done.connect_clicked(move |_| close());
        self.remote.done.connect_clicked(move |_| close_too());

        // The picker.
        let open_address = on(|me| me.picker.reveal_address());
        self.picker.type_address.connect_clicked(move |_| open_address());
        let connect = on(|me| {
            let typed = me.picker.address.text().trim().to_owned();
            if !typed.is_empty() {
                me.tv.begin(&typed);
            }
        });
        let connect_too = connect.clone();
        self.picker.connect.connect_clicked(move |_| connect());
        self.picker.address.connect_activate(move |_| connect_too());
        self.picker.address.connect_changed({
            let button = self.picker.connect.clone();
            move |entry| button.set_sensitive(!entry.text().trim().is_empty())
        });
        self.picker.list.connect_row_activated({
            let me = Rc::downgrade(self);
            move |_, row| {
                let Some(me) = me.upgrade() else { return };
                let key = row.widget_name();
                let chosen = me.tv.rows().into_iter().find(|r| r.key == key);
                if let Some(chosen) = chosen {
                    me.tv.choose(&chosen);
                }
            }
        });

        // Pairing.
        let back = on(|me| me.tv.cancel_pairing());
        self.pairing.back.connect_clicked(move |_| back());
        let pair = on(|me| {
            if let Some(code) = me.pairing.code.six() {
                me.tv.submit(&code);
            }
        });
        let pair_too = pair.clone();
        self.pairing.pair.connect_clicked(move |_| pair());
        let again = on(|me| {
            me.pairing.code.clear();
            me.tv.request_code_again();
        });
        self.pairing.show_new.connect_clicked(move |_| again());
        self.pairing.code.on_change({
            let me = Rc::downgrade(self);
            move |six| {
                let Some(me) = me.upgrade() else { return };
                let working = me.tv.pairing().is_some_and(|s| s.working);
                me.pairing.pair.set_sensitive(six.is_some() && !working);
                // The sixth digit pairs.
                if six.is_some() && !working {
                    pair_too();
                }
            }
        });

        // The remote.
        self.remote.wire(&self.tv, &self.dialog);
        let pair_again = on(|me| me.tv.pair_again());
        self.remote.pair_again.connect_clicked(move |_| pair_again());
        let another = on(|me| me.tv.open_picker());
        self.remote.another.connect_clicked(move |_| another());
        let disconnect = on(|me| {
            me.tv.disconnect();
            me.dialog.close();
        });
        self.remote.disconnect.connect_clicked(move |_| disconnect());
        self.remote.seek.connect_change_value({
            let me = Rc::downgrade(self);
            move |_, _, value| {
                let Some(me) = me.upgrade() else { return glib::Propagation::Stop };
                me.remote.dragged(value);
                let rest = glib::timeout_add_local_once(SEEK_REST, {
                    let me = Rc::downgrade(&me);
                    move || {
                        if let Some(me) = me.upgrade() {
                            me.remote.seek_rested(&me.tv);
                        }
                    }
                });
                me.remote.drag_rest.replace(Some(rest));
                glib::Propagation::Proceed
            }
        });
    }
}

/// A face's frame: its header, with no window buttons, and a column under it.
fn face(title: &adw::WindowTitle) -> (adw::ToolbarView, adw::HeaderBar, gtk::Box) {
    let header = adw::HeaderBar::new();
    header.set_show_end_title_buttons(false);
    header.set_show_start_title_buttons(false);
    header.set_title_widget(Some(title));
    let body = gtk::Box::new(gtk::Orientation::Vertical, 12);
    body.set_margin_top(6);
    body.set_margin_bottom(24);
    body.set_margin_start(18);
    body.set_margin_end(18);
    let scroll = gtk::ScrolledWindow::builder()
        .hscrollbar_policy(gtk::PolicyType::Never)
        .propagate_natural_height(true)
        .max_content_height(640)
        .child(&body)
        .build();
    let view = adw::ToolbarView::new();
    view.add_top_bar(&header);
    view.set_content(Some(&scroll));
    (view, header, body)
}

fn line() -> gtk::Label {
    let label = widgets::haze("", "");
    label.set_wrap(true);
    label
}

/// What went wrong, as the app marks a state: a vermilion dot beside plain words, never coloured
/// text. The engine's words, under a heading when the state has a name ("Code Expired").
struct ProblemLine {
    widget: gtk::Box,
    heading: gtk::Label,
    text: gtk::Label,
    shown: RefCell<Option<(String, String)>>,
}

impl ProblemLine {
    fn new() -> Self {
        let widget = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        let dot = gtk::Box::new(gtk::Orientation::Horizontal, 0);
        dot.add_css_class("mark");
        dot.add_css_class("vermilion");
        dot.set_valign(gtk::Align::Start);
        // Level with the first line's middle.
        dot.set_margin_top(7);
        widget.append(&dot);
        let words = gtk::Box::new(gtk::Orientation::Vertical, 2);
        words.set_hexpand(true);
        let heading = gtk::Label::new(None);
        heading.add_css_class("heading");
        heading.set_xalign(0.0);
        words.append(&heading);
        let text = gtk::Label::new(None);
        text.set_xalign(0.0);
        text.set_wrap(true);
        text.set_natural_wrap_mode(gtk::NaturalWrapMode::Word);
        words.append(&text);
        widget.append(&words);
        widget.set_visible(false);
        Self { widget, heading, text, shown: RefCell::default() }
    }

    /// Show `problem`, under `heading` when it has one, or nothing. A screen reader hears it
    /// once as it comes.
    fn show(&self, heading: Option<&str>, problem: Option<&str>) {
        let next = problem.map(|p| (heading.unwrap_or_default().to_owned(), p.to_owned()));
        if *self.shown.borrow() == next {
            return;
        }
        self.shown.replace(next.clone());
        self.widget.set_visible(next.is_some());
        let Some((heading, text)) = next else { return };
        self.heading.set_label(&heading);
        self.heading.set_visible(!heading.is_empty());
        self.text.set_label(&text);
        let said = if heading.is_empty() { text } else { format!("{heading}. {text}") };
        self.widget.announce(&said, gtk::AccessibleAnnouncementPriority::High);
    }
}

// The picker.

struct Picker {
    view: adw::ToolbarView,
    title: adw::WindowTitle,
    done: gtk::Button,
    line: gtk::Label,
    list: gtk::ListBox,
    searching: gtk::ListBoxRow,
    stages: gtk::Stack,
    none_line: gtk::Label,
    /// The rows as drawn, so a change that leaves them alone redraws nothing.
    drawn: RefCell<Vec<(TvRow, bool)>>,
    since: Cell<Instant>,
    type_address: gtk::Button,
    address_box: gtk::Box,
    address: gtk::Entry,
    connect: gtk::Button,
}

impl Picker {
    fn new() -> Self {
        let title = adw::WindowTitle::new("Play on TV", "");
        let (view, header, body) = face(&title);
        let done = gtk::Button::with_label("Done");
        header.pack_end(&done);
        let line = line();
        body.append(&line);

        let list = widgets::boxed_list();
        list.set_margin_top(6);
        // While no TV is listed: nothing, then the search said, then what to check.
        let stages = gtk::Stack::builder()
            .transition_type(gtk::StackTransitionType::Crossfade)
            .transition_duration(200)
            .build();
        stages.add_named(&gtk::Box::new(gtk::Orientation::Horizontal, 0), Some("quiet"));
        let looking = gtk::Box::new(gtk::Orientation::Horizontal, 10);
        looking.set_valign(gtk::Align::Center);
        let spinner = adw::Spinner::new();
        spinner.set_size_request(16, 16);
        looking.append(&spinner);
        looking.append(&widgets::haze("Looking for TVs on this network", ""));
        stages.add_named(&looking, Some("looking"));
        let none = gtk::Box::new(gtk::Orientation::Vertical, 2);
        none.set_valign(gtk::Align::Center);
        let none_title = gtk::Label::new(Some("No TVs found"));
        none_title.set_xalign(0.0);
        none.append(&none_title);
        let none_line = widgets::haze("", "caption");
        none.append(&none_line);
        stages.add_named(&none, Some("none"));
        stages.set_margin_start(12);
        stages.set_margin_end(12);
        stages.set_margin_top(8);
        stages.set_margin_bottom(8);
        let searching = gtk::ListBoxRow::builder()
            .child(&stages)
            .activatable(false)
            .selectable(false)
            .build();
        searching.set_size_request(-1, 52);
        list.append(&searching);
        body.append(&list);

        let type_address = gtk::Button::with_label("Type an Address…");
        type_address.set_halign(gtk::Align::Start);
        type_address.set_margin_top(6);
        body.append(&type_address);
        let address = gtk::Entry::builder()
            .placeholder_text("192.168.1.20:47811")
            .input_purpose(gtk::InputPurpose::Url)
            .hexpand(true)
            .build();
        address.update_property(&[gtk::accessible::Property::Label("TV address")]);
        let connect = gtk::Button::with_label("Connect");
        connect.set_sensitive(false);
        let field = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        field.append(&address);
        field.append(&connect);
        let address_box = gtk::Box::new(gtk::Orientation::Vertical, 6);
        address_box.set_margin_top(6);
        address_box.append(&field);
        address_box.append(&widgets::haze(
            "The TV shows its address on its Pair a phone screen.",
            "caption",
        ));
        address_box.set_visible(false);
        body.append(&address_box);

        Self {
            view,
            title,
            done,
            line,
            list,
            searching,
            stages,
            none_line,
            drawn: RefCell::default(),
            since: Cell::new(Instant::now()),
            type_address,
            address_box,
            address,
            connect,
        }
    }

    fn reveal_address(&self) {
        self.type_address.set_visible(false);
        self.address_box.set_visible(true);
        self.address.grab_focus();
    }

    fn update(&self, tv: &Tv, came: bool) {
        let request = tv.request();
        self.title.set_title(if request.is_some() { "Play on TV" } else { "Pair a TV" });
        self.line.set_label(&match &request {
            None => {
                "Open anistream on the TV. It needs to be on the same Wi-Fi as this computer."
                    .to_owned()
            }
            Some(r) if r.title.is_empty() => format!("Episode {}", r.episode),
            Some(r) => format!("{}, episode {}", r.title, r.episode),
        });
        self.none_line.set_label(if request.is_none() {
            "Check that anistream is open on the TV."
        } else {
            "Check that anistream is open on the TV, on the same Wi-Fi as this computer."
        });
        if came {
            self.address.set_text("");
            self.address_box.set_visible(false);
            self.type_address.set_visible(true);
        }
        let connected = tv.connected();
        let rows: Vec<(TvRow, bool)> = tv
            .rows()
            .into_iter()
            .map(|row| {
                let on = row.fingerprint.is_some() && row.fingerprint == connected;
                (row, on)
            })
            .collect();
        if *self.drawn.borrow() != rows {
            while let Some(child) = self.searching.next_sibling() {
                self.list.remove(&child);
            }
            for (row, on) in &rows {
                self.list.append(&tv_row(row, *on));
            }
            self.drawn.replace(rows);
        }
        self.show_searching(tv);
    }

    /// The searching row, while no TV is listed.
    fn show_searching(&self, _tv: &Tv) {
        let empty = self.drawn.borrow().is_empty();
        self.searching.set_visible(empty);
        let waited = self.since.get().elapsed();
        let stage = if waited < QUIET_FOR {
            "quiet"
        } else if waited < GIVE_UP_AFTER {
            "looking"
        } else {
            "none"
        };
        if self.stages.visible_child_name().as_deref() != Some(stage) {
            self.stages.set_visible_child_name(stage);
            if stage == "none" && empty {
                self.stages.announce(
                    &format!("No TVs found. {}", self.none_line.label()),
                    gtk::AccessibleAnnouncementPriority::Medium,
                );
            }
        }
    }
}

/// A row of the picker: the TV, what the engine says under it, and what choosing it does.
fn tv_row(row: &TvRow, connected: bool) -> adw::ActionRow {
    let line = adw::ActionRow::builder()
        .title(&row.name)
        .subtitle(row.kind.detail())
        .use_markup(false)
        .activatable(row.kind != TvRowKind::NeedsUpdate)
        .build();
    line.set_widget_name(&row.key);
    match row.kind {
        TvRowKind::New => {
            let pair = gtk::Label::new(Some("Pair"));
            pair.add_css_class("heading");
            line.add_suffix(&pair);
            line.upcast_ref::<gtk::Widget>().update_property(&[
                gtk::accessible::Property::Description("Pairs with this TV"),
            ]);
        }
        TvRowKind::Paired | TvRowKind::PairedAway if connected => {
            let check = gtk::Image::from_icon_name("object-select-symbolic");
            check.update_property(&[gtk::accessible::Property::Label("Connected")]);
            line.add_suffix(&check);
        }
        TvRowKind::Paired | TvRowKind::PairedAway => line.add_suffix(&widgets::chevron()),
        TvRowKind::NeedsUpdate => line.set_sensitive(false),
    }
    line
}

// Pairing.

struct Pairing {
    view: adw::ToolbarView,
    title: adw::WindowTitle,
    back: gtk::Button,
    pair: gtk::Button,
    spinner: adw::Spinner,
    line: gtk::Label,
    code: Rc<CodeBoxes>,
    show_new: gtk::Button,
    problem: ProblemLine,
    shown_problem: RefCell<Option<String>>,
}

impl Pairing {
    fn new() -> Self {
        let title = adw::WindowTitle::new("Pair a TV", "");
        let (view, header, body) = face(&title);
        let back = gtk::Button::with_label("Back");
        header.pack_start(&back);
        let pair = gtk::Button::with_label("Pair");
        pair.add_css_class("suggested-action");
        pair.set_sensitive(false);
        header.pack_end(&pair);
        let spinner = adw::Spinner::new();
        spinner.set_size_request(16, 16);
        header.pack_end(&spinner);
        let line = line();
        body.append(&line);
        let code = CodeBoxes::new();
        code.widget.set_margin_top(12);
        code.widget.set_margin_bottom(6);
        body.append(&code.widget);
        let show_new = gtk::Button::with_label("Show New Code");
        show_new.set_halign(gtk::Align::Center);
        body.append(&show_new);
        let problem = ProblemLine::new();
        problem.widget.set_margin_top(6);
        body.append(&problem.widget);
        Self {
            view,
            title,
            back,
            pair,
            spinner,
            line,
            code,
            show_new,
            problem,
            shown_problem: RefCell::default(),
        }
    }

    fn update(&self, tv: &Tv, came: bool) {
        let Some(step) = tv.pairing() else { return };
        let typing = step.needs_code && !step.tv_name.is_empty();
        if typing {
            self.title.set_title(&format!("Pair with {}", step.tv_name));
            self.line.set_label(&format!(
                "Type the six digits {} shows. After this, this computer connects by itself.",
                step.tv_name
            ));
        } else {
            self.title.set_title(&if step.tv_name.is_empty() {
                "Connecting to the TV".to_owned()
            } else {
                format!("Pairing with {}", step.tv_name)
            });
            self.line.set_label("This takes a moment.");
        }
        // Once a try has failed, the problem says what happened in its place.
        self.line.set_visible(typing || step.working || step.problem.is_none());
        if came {
            self.code.clear();
            self.shown_problem.replace(None);
        }
        // A code that ran out takes no digits until the TV shows a new one.
        let expired = step.expired;
        self.code.widget.set_visible(typing);
        self.code.set_sensitive(!step.working && !expired);
        self.show_new.set_visible(typing);
        self.show_new.set_sensitive(!step.working);
        self.show_new.set_label(if expired { "Get a New Code" } else { "Show New Code" });
        if expired {
            self.show_new.add_css_class("suggested-action");
        } else {
            self.show_new.remove_css_class("suggested-action");
        }
        self.pair.set_visible(typing);
        self.pair.set_sensitive(self.code.six().is_some() && !step.working && !expired);
        self.spinner.set_visible(step.working);
        // A try that failed: the code goes, ready for the next, or for a new code.
        if step.problem.is_some() && *self.shown_problem.borrow() != step.problem {
            self.code.clear();
            if expired {
                self.show_new.grab_focus();
            } else {
                self.code.focus();
            }
        }
        self.shown_problem.replace(step.problem.clone());
        self.problem.show(expired.then_some("Code Expired"), step.problem.as_deref());
        if typing && !step.working && !expired && came {
            self.code.focus();
        }
    }
}

/// Told the six digits once typed, or none while fewer.
type OnCode = Box<dyn Fn(Option<String>)>;

/// Six digits in boxes, as the code reads on the TV, typed into one field over them.
struct CodeBoxes {
    widget: gtk::Overlay,
    text: gtk::Text,
    boxes: Vec<gtk::Label>,
    focused: Cell<bool>,
    filling: Cell<bool>,
    changed: RefCell<Option<OnCode>>,
}

impl CodeBoxes {
    fn new() -> Rc<Self> {
        let row = gtk::Box::new(gtk::Orientation::Horizontal, 6);
        row.set_halign(gtk::Align::Center);
        let mut boxes = Vec::with_capacity(6);
        for index in 0..6 {
            let digit = gtk::Label::new(None);
            digit.add_css_class("code-box");
            digit.add_css_class("numeric");
            digit.set_size_request(40, 46);
            row.append(&digit);
            if index == 2 {
                let gap = gtk::Box::new(gtk::Orientation::Horizontal, 0);
                gap.set_size_request(10, -1);
                row.append(&gap);
            }
            boxes.push(digit);
        }
        row.set_can_target(false);
        row.update_property(&[gtk::accessible::Property::Label("Pairing code")]);
        let text = gtk::Text::builder()
            .input_purpose(gtk::InputPurpose::Digits)
            .max_length(12)
            .opacity(0.0)
            .can_target(false)
            .build();
        text.update_property(&[gtk::accessible::Property::Label("Pairing code")]);
        let widget = gtk::Overlay::new();
        widget.set_child(Some(&row));
        widget.add_overlay(&text);
        let me = Rc::new(Self {
            widget,
            text,
            boxes,
            focused: Cell::new(false),
            filling: Cell::new(false),
            changed: RefCell::default(),
        });
        let click = gtk::GestureClick::new();
        click.connect_pressed({
            let me = Rc::downgrade(&me);
            move |_, _, _, _| {
                if let Some(me) = me.upgrade() {
                    me.focus();
                }
            }
        });
        me.widget.add_controller(click);
        let focus = gtk::EventControllerFocus::new();
        focus.connect_enter({
            let me = Rc::downgrade(&me);
            move |_| {
                if let Some(me) = me.upgrade() {
                    me.focused.set(true);
                    me.draw();
                }
            }
        });
        focus.connect_leave({
            let me = Rc::downgrade(&me);
            move |_| {
                if let Some(me) = me.upgrade() {
                    me.focused.set(false);
                    me.draw();
                }
            }
        });
        me.text.add_controller(focus);
        me.text.connect_changed({
            let me = Rc::downgrade(&me);
            move |text| {
                let Some(me) = me.upgrade() else { return };
                if me.filling.get() {
                    return;
                }
                let typed = text.text();
                let kept: String = typed.chars().filter(char::is_ascii_digit).take(6).collect();
                if kept != typed.as_str() {
                    me.filling.set(true);
                    text.set_text(&kept);
                    text.set_position(-1);
                    me.filling.set(false);
                }
                me.draw();
                if let Some(changed) = me.changed.borrow().as_ref() {
                    changed(me.six());
                }
            }
        });
        me.draw();
        me
    }

    fn on_change(&self, f: impl Fn(Option<String>) + 'static) {
        self.changed.replace(Some(Box::new(f)));
    }

    /// The digits, once there are six.
    fn six(&self) -> Option<String> {
        let digits: String = self.text.text().chars().filter(char::is_ascii_digit).collect();
        (digits.len() == 6).then_some(digits)
    }

    fn clear(&self) {
        self.filling.set(true);
        self.text.set_text("");
        self.filling.set(false);
        self.draw();
    }

    fn focus(&self) {
        self.text.grab_focus();
    }

    fn set_sensitive(&self, on: bool) {
        self.text.set_editable(on);
        self.widget.set_opacity(if on { 1.0 } else { 0.6 });
    }

    fn draw(&self) {
        let digits: Vec<char> = self.text.text().chars().collect();
        for (index, label) in self.boxes.iter().enumerate() {
            label.set_label(&digits.get(index).map(char::to_string).unwrap_or_default());
            if self.focused.get() && index == digits.len() {
                label.add_css_class("current");
            } else {
                label.remove_css_class("current");
            }
        }
    }
}

// The remote.

struct Remote {
    view: adw::ToolbarView,
    title: adw::WindowTitle,
    done: gtk::Button,
    status: gtk::Box,
    shown_status: RefCell<(String, &'static str)>,
    now: gtk::Box,
    cover: gtk::Box,
    cover_of: Cell<Option<u32>>,
    show: gtk::Label,
    episode: gtk::Label,
    skip: gtk::Button,
    seek: gtk::Scale,
    position: gtk::Label,
    remaining: gtk::Label,
    /// A drag on the line, until it rests and is sent; then where it let go, until the TV says.
    dragging: Cell<Option<f64>>,
    drag_rest: RefCell<Option<glib::SourceId>>,
    held: Cell<Option<(f64, u64, Instant)>>,
    transport: gtk::Box,
    play: gtk::Button,
    rows: gtk::ListBox,
    subtitles: gtk::MenuButton,
    audio: gtk::MenuButton,
    tracks_drawn: RefCell<(Vec<TrackInfo>, Vec<TrackInfo>)>,
    subtitle_action: gio::SimpleAction,
    audio_action: gio::SimpleAction,
    volume: adw::ActionRow,
    nothing: gtk::Label,
    search: gtk::Entry,
    filling_search: Rc<Cell<bool>>,
    problem: ProblemLine,
    pair_again: gtk::Button,
    another: gtk::Button,
    disconnect: gtk::Button,
    volume_down: gtk::Button,
    volume_up: gtk::Button,
    back10: gtk::Button,
    forward10: gtk::Button,
    next: gtk::Button,
}

fn glyph(icon: &str, tip: &str) -> gtk::Button {
    let button = gtk::Button::from_icon_name(icon);
    button.set_tooltip_text(Some(tip));
    button.add_css_class("flat");
    button.add_css_class("circular");
    button.set_valign(gtk::Align::Center);
    button
}

impl Remote {
    fn new() -> Self {
        let title = adw::WindowTitle::new("TV", "");
        let (view, header, body) = face(&title);
        let done = gtk::Button::with_label("Done");
        header.pack_end(&done);
        let status = gtk::Box::new(gtk::Orientation::Horizontal, 0);
        body.append(&status);

        let now = gtk::Box::new(gtk::Orientation::Vertical, 10);
        now.set_margin_top(6);
        let head = gtk::Box::new(gtk::Orientation::Horizontal, 12);
        let cover = gtk::Box::new(gtk::Orientation::Horizontal, 0);
        cover.set_valign(gtk::Align::Center);
        head.append(&cover);
        let text = gtk::Box::new(gtk::Orientation::Vertical, 2);
        text.set_valign(gtk::Align::Center);
        text.set_hexpand(true);
        let show = widgets::show_title("", "row");
        show.set_wrap(false);
        show.set_ellipsize(gtk::pango::EllipsizeMode::End);
        text.append(&show);
        let episode = widgets::haze("", "caption");
        episode.set_wrap(false);
        episode.set_ellipsize(gtk::pango::EllipsizeMode::End);
        text.append(&episode);
        head.append(&text);
        let skip = gtk::Button::with_label("Skip Opening");
        skip.set_valign(gtk::Align::Center);
        head.append(&skip);
        now.append(&head);

        let seek = gtk::Scale::with_range(gtk::Orientation::Horizontal, 0.0, 1.0, 1.0);
        seek.set_draw_value(false);
        seek.set_hexpand(true);
        seek.set_increments(10.0, 60.0);
        seek.update_property(&[gtk::accessible::Property::Label("Position")]);
        let times = gtk::Box::new(gtk::Orientation::Horizontal, 0);
        let position = widgets::haze("0:00", "caption");
        position.add_css_class("numeric");
        position.set_hexpand(true);
        let remaining = widgets::haze("", "caption");
        remaining.add_css_class("numeric");
        remaining.set_xalign(1.0);
        times.append(&position);
        times.append(&remaining);
        let seek_line = gtk::Box::new(gtk::Orientation::Vertical, 0);
        seek_line.append(&seek);
        seek_line.append(&times);
        now.append(&seek_line);

        let back10 = glyph("media-seek-backward-symbolic", "Back 10 Seconds");
        let play = glyph("media-playback-pause-symbolic", "Pause");
        play.add_css_class("tv-play");
        let forward10 = glyph("media-seek-forward-symbolic", "Forward 10 Seconds");
        let next = glyph("media-skip-forward-symbolic", "Next Episode");
        let transport = gtk::Box::new(gtk::Orientation::Horizontal, 16);
        transport.set_halign(gtk::Align::Center);
        for button in [&back10, &play, &forward10, &next] {
            transport.append(button);
        }
        now.append(&transport);

        let rows = widgets::boxed_list();
        rows.set_margin_top(6);
        let menu_button = || {
            let button = gtk::MenuButton::builder().valign(gtk::Align::Center).build();
            button.add_css_class("flat");
            button
        };
        let subtitles = menu_button();
        subtitles.update_property(&[gtk::accessible::Property::Label("Subtitles")]);
        let subtitle_row = adw::ActionRow::builder().title("Subtitles").build();
        subtitle_row.add_suffix(&subtitles);
        subtitle_row.set_activatable_widget(Some(&subtitles));
        rows.append(&subtitle_row);
        let audio = menu_button();
        audio.update_property(&[gtk::accessible::Property::Label("Audio")]);
        let audio_row = adw::ActionRow::builder().title("Audio").build();
        audio_row.add_suffix(&audio);
        audio_row.set_activatable_widget(Some(&audio));
        rows.append(&audio_row);
        let volume = adw::ActionRow::builder().title("TV Volume").build();
        // "40 of 100" in even digits, as the times and the code.
        volume.add_css_class("numeric");
        let volume_down = glyph("audio-volume-low-symbolic", "Volume Down");
        let volume_up = glyph("audio-volume-high-symbolic", "Volume Up");
        volume.add_suffix(&volume_down);
        volume.add_suffix(&volume_up);
        rows.append(&volume);
        now.append(&rows);
        body.append(&now);

        let nothing = line();
        nothing.set_label("Choose Play on TV on an episode, or search on the TV from here.");
        body.append(&nothing);

        let search = gtk::Entry::builder()
            .placeholder_text("Search on the TV")
            .primary_icon_name("system-search-symbolic")
            .margin_top(6)
            .build();
        search.update_property(&[gtk::accessible::Property::Label("Search on the TV")]);
        body.append(&search);
        let problem = ProblemLine::new();
        body.append(&problem.widget);

        let foot = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        foot.set_margin_top(6);
        let pair_again = gtk::Button::with_label("Pair Again");
        let another = gtk::Button::with_label("Choose Another TV…");
        let disconnect = gtk::Button::with_label("Disconnect");
        foot.append(&pair_again);
        foot.append(&another);
        foot.append(&disconnect);
        body.append(&foot);

        let radio = |name: &str| {
            gio::SimpleAction::new_stateful(
                name,
                Some(glib::VariantTy::STRING),
                &String::new().to_variant(),
            )
        };
        Self {
            view,
            title,
            done,
            status,
            shown_status: RefCell::new((String::new(), "")),
            now,
            cover,
            cover_of: Cell::new(None),
            show,
            episode,
            skip,
            seek,
            position,
            remaining,
            dragging: Cell::new(None),
            drag_rest: RefCell::default(),
            held: Cell::new(None),
            transport,
            play,
            rows,
            subtitles,
            audio,
            tracks_drawn: RefCell::default(),
            subtitle_action: radio("subtitle"),
            audio_action: radio("audio"),
            volume,
            nothing,
            search,
            filling_search: Rc::default(),
            problem,
            pair_again,
            another,
            disconnect,
            volume_down,
            volume_up,
            back10,
            forward10,
            next,
        }
    }

    fn wire(&self, tv: &Rc<Tv>, dialog: &adw::Dialog) {
        let send = |button: &gtk::Button, command: RemoteCommand| {
            let tv = Rc::downgrade(tv);
            button.connect_clicked(move |_| {
                if let Some(tv) = tv.upgrade() {
                    tv.send(command.clone());
                }
            });
        };
        send(&self.back10, RemoteCommand::Seek { by: -10.0 });
        send(&self.forward10, RemoteCommand::Seek { by: 10.0 });
        send(&self.play, RemoteCommand::TogglePause);
        send(&self.next, RemoteCommand::Next);
        send(&self.skip, RemoteCommand::Skip);
        send(&self.volume_down, RemoteCommand::Volume { step: -VOLUME_STEP });
        send(&self.volume_up, RemoteCommand::Volume { step: VOLUME_STEP });

        // The track menus' radio items.
        let group = gio::SimpleActionGroup::new();
        for (action, kind) in
            [(&self.subtitle_action, Track::Subtitle), (&self.audio_action, Track::Audio)]
        {
            let tv = Rc::downgrade(tv);
            action.connect_activate(move |action, target| {
                let (Some(tv), Some(target)) = (tv.upgrade(), target) else { return };
                let Some(chosen) = target.str() else { return };
                action.set_state(target);
                let id = chosen.parse::<i64>().ok();
                tv.send(RemoteCommand::Track { kind, id });
            });
            group.add_action(action);
        }
        dialog.insert_action_group("tv", Some(&group));

        // Typed on the TV as it is typed here; Enter searches.
        self.search.connect_changed({
            let tv = Rc::downgrade(tv);
            let filling = Rc::clone(&self.filling_search);
            move |entry| {
                if filling.get() {
                    return;
                }
                if let Some(tv) = tv.upgrade() {
                    tv.send(RemoteCommand::Text { text: entry.text().into(), submit: false });
                }
            }
        });
        self.search.connect_activate({
            let tv = Rc::downgrade(tv);
            move |entry| {
                if let Some(tv) = tv.upgrade() {
                    tv.send(RemoteCommand::Text { text: entry.text().into(), submit: true });
                }
            }
        });

        // Space plays and pauses, and Left and Right step 10 seconds, as a remote's keys do.
        // Typing in the search, or a menu open, keeps its keys; Enter still presses a button.
        let keys = gtk::EventControllerKey::new();
        keys.set_propagation_phase(gtk::PropagationPhase::Capture);
        keys.connect_key_pressed({
            let tv = Rc::downgrade(tv);
            let view = self.view.downgrade();
            move |_, key, _, modifiers| {
                let command = match key {
                    gdk::Key::space => RemoteCommand::TogglePause,
                    gdk::Key::Left => RemoteCommand::Seek { by: -10.0 },
                    gdk::Key::Right => RemoteCommand::Seek { by: 10.0 },
                    _ => return glib::Propagation::Proceed,
                };
                if !modifiers.is_empty() {
                    return glib::Propagation::Proceed;
                }
                let focus = view.upgrade().and_then(|v| v.root()).and_then(|r| r.focus());
                let keeps_keys = focus.is_some_and(|f| {
                    f.is::<gtk::Text>() || f.ancestor(gtk::Popover::static_type()).is_some()
                });
                match tv.upgrade() {
                    Some(tv) if !keeps_keys && tv.live() && tv.now().is_some() => {
                        tv.send(command);
                        glib::Propagation::Stop
                    }
                    _ => glib::Propagation::Proceed,
                }
            }
        });
        self.view.add_controller(keys);
    }

    fn update(&self, ctx: &Rc<Ctx>, came: bool) {
        let tv = &ctx.tv;
        let card = tv.card();
        let live = tv.live();
        let name = card.as_ref().map_or_else(|| "TV".to_owned(), |c| c.tv_name.clone());
        self.title.set_title(&name);
        let line = card
            .as_ref()
            .and_then(|c| c.phase_line.clone())
            .unwrap_or_else(|| "Connected".to_owned());
        let mark = match card.as_ref().map(|c| c.phase) {
            Some(LinkPhase::Connected) => "water",
            Some(LinkPhase::Unpaired | LinkPhase::KeyChanged) => "vermilion",
            _ => "haze",
        };
        if *self.shown_status.borrow() != (line.clone(), mark) {
            while let Some(child) = self.status.first_child() {
                self.status.remove(&child);
            }
            self.status.append(&widgets::standing(&line, mark));
            self.shown_status.replace((line, mark));
        }
        if came {
            self.filling_search.set(true);
            self.search.set_text("");
            self.filling_search.set(false);
        }

        let now = tv.now();
        self.now.set_visible(now.is_some());
        self.nothing.set_visible(
            now.is_none() && card.as_ref().is_some_and(|c| c.phase == LinkPhase::Connected),
        );
        if let Some(now) = &now {
            self.draw_now(ctx, now, live);
        }
        self.search.set_sensitive(live);
        self.problem.show(None, tv.problem().as_deref());
        let lost = tv.lost();
        self.pair_again.set_visible(lost);
        self.another.set_visible(!lost);
        self.show_playhead(tv);
    }

    fn draw_now(&self, ctx: &Rc<Ctx>, now: &NowCard, live: bool) {
        let id = now.id.get();
        if self.cover_of.replace(Some(id)) != Some(id) {
            while let Some(child) = self.cover.first_child() {
                self.cover.remove(&child);
            }
            if let Some(entry) = ctx.hub.entry(now.id) {
                self.cover.append(&widgets::cover(&ctx.hub, &entry, 36, 54));
            }
        }
        let title = ctx.hub.entry(now.id).map_or_else(|| now.title.clone(), |e| e.title);
        self.show.set_label(if now.title.is_empty() { &title } else { &now.title });
        self.episode.set_label(if now.loading && now.duration.is_none() {
            "Starting on the TV"
        } else {
            &now.episode_line
        });
        match &now.skip {
            Some(skip) => {
                self.skip.set_label(&header_case(&skip.label));
                self.skip.set_visible(true);
            }
            None => self.skip.set_visible(false),
        }
        self.skip.set_sensitive(live);
        let controls = live && !now.loading;
        self.transport.set_sensitive(controls);
        let (icon, tip) = if now.paused {
            ("media-playback-start-symbolic", "Play")
        } else {
            ("media-playback-pause-symbolic", "Pause")
        };
        self.play.set_icon_name(icon);
        self.play.set_tooltip_text(Some(tip));
        self.seek.set_sensitive(controls && now.duration.is_some());
        if let Some(duration) = now.duration {
            self.seek.set_range(0.0, duration.max(1.0));
        }

        if *self.tracks_drawn.borrow() != (now.subtitles.clone(), now.audio.clone()) {
            self.subtitles.set_menu_model(Some(&track_menu(&now.subtitles, Track::Subtitle)));
            self.audio.set_menu_model(Some(&track_menu(&now.audio, Track::Audio)));
            self.tracks_drawn.replace((now.subtitles.clone(), now.audio.clone()));
        }
        let chosen_subtitle = if now.subtitles_off {
            "off".to_owned()
        } else {
            now.subtitles
                .iter()
                .find(|t| t.selected)
                .map(|t| t.id.to_string())
                .unwrap_or_default()
        };
        self.subtitle_action.set_state(&chosen_subtitle.to_variant());
        let chosen_audio =
            now.audio.iter().find(|t| t.selected).map(|t| t.id.to_string()).unwrap_or_default();
        self.audio_action.set_state(&chosen_audio.to_variant());
        self.subtitles.set_label(&chosen(&now.subtitles, now.subtitles_off, Track::Subtitle));
        self.audio.set_label(&chosen(&now.audio, false, Track::Audio));
        self.subtitles.set_sensitive(live && !now.subtitles.is_empty());
        self.audio.set_sensitive(live && !now.audio.is_empty());
        self.volume.set_subtitle(
            &now.volume.map(|v| format!("{} of 100", v.round() as i64)).unwrap_or_default(),
        );
        self.volume.set_sensitive(live);
        self.rows.set_opacity(if live { 1.0 } else { 0.6 });
    }

    /// Moved by the viewer: the line follows, and once the drag rests the TV is told.
    fn dragged(&self, value: f64) {
        self.dragging.set(Some(value));
        self.position.set_label(&words::clock(value));
        if let Some(id) = self.drag_rest.take() {
            id.remove();
        }
    }

    /// The drag rested: the TV seeks there, and the line holds it until the TV says.
    fn seek_rested(&self, tv: &Tv) {
        self.drag_rest.take();
        let Some(position) = self.dragging.take() else { return };
        let rev = tv.card().map_or(0, |c| c.rev);
        self.held.set(Some((position, rev, Instant::now())));
        tv.send(RemoteCommand::SeekTo { position });
    }

    /// The TV's playhead, moved on the computer's clock, unless a drag or a seek holds it.
    fn show_playhead(&self, tv: &Tv) {
        let Some(now) = tv.now() else { return };
        let rev = tv.card().map_or(0, |c| c.rev);
        if let Some((_, at_rev, since)) = self.held.get()
            && (at_rev != rev || since.elapsed() > SEEK_HOLD)
        {
            self.held.set(None);
        }
        let position = self
            .dragging
            .get()
            .or(self.held.get().map(|h| h.0))
            .or_else(|| tv.playhead())
            .unwrap_or(now.position);
        // Before the TV knows the length, the bar waits empty rather than reading full.
        if self.dragging.get().is_none() {
            self.seek.set_value(if now.duration.is_some() { position } else { 0.0 });
        }
        self.position.set_label(&words::clock(position));
        self.remaining.set_label(
            &now.duration
                .map(|d| format!("\u{2212}{}", words::clock((d - position).max(0.0))))
                .unwrap_or_default(),
        );
        self.seek
            .update_property(&[gtk::accessible::Property::ValueText(&words::clock(position))]);
    }
}

/// "Skip opening" as a button says it: "Skip Opening".
fn header_case(label: &str) -> String {
    label
        .split(' ')
        .map(|word| {
            let mut chars = word.chars();
            chars.next().map_or_else(String::new, |c| c.to_uppercase().chain(chars).collect())
        })
        .collect::<Vec<_>>()
        .join(" ")
}

fn kind_of(track: Track) -> TrackKind {
    match track {
        Track::Audio => TrackKind::Audio,
        Track::Subtitle => TrackKind::Subtitle,
    }
}

fn track_label(track: &TrackInfo, kind: Track) -> String {
    crate::player::tracks::label(
        track.language.as_deref(),
        track.title.as_deref(),
        track.id,
        kind_of(kind),
    )
}

/// What the menu button says: the track on, "Off", or "None" when the TV says of none.
fn chosen(tracks: &[TrackInfo], off: bool, kind: Track) -> String {
    if off {
        return "Off".to_owned();
    }
    tracks
        .iter()
        .find(|t| t.selected)
        .map_or_else(|| "None".to_owned(), |t| track_label(t, kind))
}

fn track_menu(tracks: &[TrackInfo], kind: Track) -> gio::Menu {
    let (action, menu) = match kind {
        Track::Subtitle => ("tv.subtitle", gio::Menu::new()),
        Track::Audio => ("tv.audio", gio::Menu::new()),
    };
    if kind == Track::Subtitle {
        menu.append_item(&widgets::menu_item("Off", action, "off"));
    }
    for track in tracks {
        menu.append_item(&widgets::menu_item(
            &track_label(track, kind),
            action,
            &track.id.to_string(),
        ));
    }
    menu
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_skip_reads_in_header_capitals() {
        assert_eq!(header_case("Skip opening"), "Skip Opening");
        assert_eq!(header_case("Skip ending"), "Skip Ending");
    }

    #[test]
    fn the_track_button_says_off_none_or_the_track() {
        let track = |id, selected| TrackInfo {
            id,
            kind: Track::Subtitle,
            title: Some("Signs".into()),
            language: None,
            selected,
        };
        assert_eq!(chosen(&[track(1, false)], true, Track::Subtitle), "Off");
        assert_eq!(chosen(&[track(1, false)], false, Track::Subtitle), "None");
        assert_eq!(chosen(&[track(1, false), track(2, true)], false, Track::Subtitle), "Signs");
    }
}
