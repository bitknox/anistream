//! The TV sheet, as the Mac's: the picker that finds TVs and pairs with one, the six digits a TV
//! shows, and the remote for the TV driven. One dialog, which a narrow window shows as a bottom
//! sheet; its face follows the model. Every word about the link and what went wrong is the
//! catalogue's, chosen here from what the engine says happened.

use std::{
    cell::{Cell, RefCell},
    rc::Rc,
    time::{Duration, Instant},
};

use adw::prelude::*;
use anistream_engine::{
    cast::protocol::VolumeControl,
    remote::{
        protocol::{
            command::{RemoteCommand, Track},
            state::TrackInfo,
        },
        sender::{CastRow, CastUpNext, LinkPhase, NowCard, Problem, TvRow, TvRowKind},
    },
};
use anistream_i18n::t;
use anistream_model::{command::Command, hosted::TrackKind};
use gtk::{gdk, gio, glib};

use super::{
    Sheet as Shown, Tv,
    cast::{CastView, Offer, Stage},
};
use crate::{
    say, widgets,
    window::{Ctx, Size},
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
/// A volume dragged on the scale is sent once the drag rests this long.
const VOLUME_REST: Duration = Duration::from_millis(200);
/// Where a volume let go is held on the scale until the TV says where it went, or this long.
const VOLUME_HOLD: Duration = Duration::from_millis(1500);

/// Open the sheet whenever the model asks for it, and close it when the model lets it go.
pub fn attach(ctx: &Rc<Ctx>) {
    let showing: Rc<RefCell<Option<Rc<Dialog>>>> = Rc::default();
    // Someone else took the TV: the card has gone, and the notice stays 8 s with Play here.
    ctx.tv.on_taken_over({
        let ctx = Rc::downgrade(ctx);
        move |text| {
            if let Some(ctx) = ctx.upgrade() {
                ctx.toast_with(text, &t::tv_cast_play_here(), "win.tv-play-here");
            }
        }
    });
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
            .title(t::tv_play_on())
            .follows_content_size(true)
            .child(&stack)
            .build();
        // 440 wide in a window with room, and the window's own width as a bottom sheet: kept
        // as the window is resized with the sheet up.
        ctx.on_size({
            let stack = stack.downgrade();
            move |size| {
                if let Some(stack) = stack.upgrade() {
                    stack.set_size_request(if size == Size::Narrow { -1 } else { 440 }, -1);
                }
            }
        });
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
                    Some(Face::Remote) => {
                        // A cast's starting line counts the seconds.
                        if me.tv.casting() {
                            me.tv.cast_tick();
                            me.remote.update(&me.ctx, false);
                        }
                        me.remote.show_playhead(&me.tv);
                    }
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
                // The remote itself, not a control, so Space and the arrows reach the TV
                // at once; Tab goes on to the controls.
                Face::Remote => {
                    self.remote.view.grab_focus();
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

        self.picker.cast_list.connect_row_activated({
            let me = Rc::downgrade(self);
            move |_, row| {
                let Some(me) = me.upgrade() else { return };
                let key = row.widget_name();
                let chosen = me.tv.cast_rows().into_iter().find(|r| r.key == key);
                if let Some(chosen) = chosen {
                    me.tv.choose_cast(&chosen);
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
        self.remote.volume_scale.connect_change_value({
            let me = Rc::downgrade(self);
            move |_, _, value| {
                let Some(me) = me.upgrade() else { return glib::Propagation::Stop };
                me.remote.volume_dragged(value);
                let rest = glib::timeout_add_local_once(VOLUME_REST, {
                    let me = Rc::downgrade(&me);
                    move || {
                        if let Some(me) = me.upgrade() {
                            me.remote.volume_rested(&me.tv);
                        }
                    }
                });
                me.remote.volume_rest.replace(Some(rest));
                glib::Propagation::Proceed
            }
        });

        // The cast's card.
        self.remote.mute.connect_clicked({
            let tv = Rc::downgrade(&self.tv);
            move |_| {
                if let Some(tv) = tv.upgrade()
                    && let Some(view) = tv.cast_view()
                {
                    tv.cast_send(RemoteCommand::Mute { muted: !view.muted });
                }
            }
        });
        let play_here = on(|me| {
            if let Some((id, episode)) = me.tv.cast_play_here() {
                me.ctx.hub.send(Command::play(id, episode));
            }
        });
        self.remote.play_here.connect_clicked(move |_| play_here());
        let stop = on(|me| me.tv.cast_stop());
        self.remote.stop.connect_clicked(move |_| stop());
        let try_again = on(|me| me.tv.cast_try_again());
        self.remote.try_again.connect_clicked(move |_| try_again());
        let choose_another = on(|me| me.tv.cast_choose_another());
        self.remote.choose_another.connect_clicked(move |_| choose_another());
        let choose_source = on(|me| {
            if let Some((id, episode)) = me.tv.cast_choose_source() {
                crate::party::source::choose(&me.ctx, id, &episode);
            }
        });
        self.remote.choose_source.connect_clicked(move |_| choose_source());
        let undo = on(|me| me.tv.cast_undo_skip());
        self.remote.undo.connect_clicked(move |_| undo());
        let cancel_next = on(|me| me.tv.cast_cancel_next());
        self.remote.up_next_cancel.connect_clicked(move |_| cancel_next());
        let play_next = on(|me| me.tv.cast_play_next());
        self.remote.up_next_play.connect_clicked(move |_| play_next());
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
        let said =
            if heading.is_empty() { text } else { t::tv_problem_announce(&heading, &text) };
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
    /// Over the two lists once both have TVs.
    network_heading: gtk::Label,
    cast_heading: gtk::Label,
    cast_list: gtk::ListBox,
    drawn_cast: RefCell<Vec<CastRow>>,
    searching: gtk::ListBoxRow,
    stages: gtk::Stack,
    none_line: gtk::Label,
    /// "TVs with Chromecast built in show here too.", when a TV can be cast to.
    cast_line: gtk::Label,
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
        let title = adw::WindowTitle::new(&t::tv_play_on(), "");
        let (view, header, body) = face(&title);
        let done = gtk::Button::with_label(&t::action_done());
        header.pack_end(&done);
        let line = line();
        body.append(&line);

        let group_heading = |text: String| {
            let heading = gtk::Label::new(Some(&text));
            heading.add_css_class("heading");
            heading.set_xalign(0.0);
            heading.set_margin_top(6);
            heading.set_visible(false);
            heading
        };
        let network_heading = group_heading(t::tv_network_group());
        body.append(&network_heading);
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
        looking.append(&widgets::haze(&t::tv_looking(), ""));
        stages.add_named(&looking, Some("looking"));
        let none = gtk::Box::new(gtk::Orientation::Vertical, 2);
        none.set_valign(gtk::Align::Center);
        let none_title = gtk::Label::new(Some(&t::tv_none_found()));
        none_title.set_xalign(0.0);
        none.append(&none_title);
        let none_line = widgets::haze("", "caption");
        none.append(&none_line);
        let cast_line = widgets::haze(&t::tv_cast_none_found(), "caption");
        none.append(&cast_line);
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
        let cast_heading = group_heading(t::tv_cast_group());
        body.append(&cast_heading);
        let cast_list = widgets::boxed_list();
        cast_list.set_visible(false);
        body.append(&cast_list);

        let type_address = gtk::Button::with_label(&t::tv_type_address());
        type_address.set_halign(gtk::Align::Start);
        type_address.set_margin_top(6);
        body.append(&type_address);
        let address = gtk::Entry::builder()
            .placeholder_text("192.168.1.20:47811") // i18n-skip
            .input_purpose(gtk::InputPurpose::Url)
            .hexpand(true)
            .build();
        address.update_property(&[gtk::accessible::Property::Label(&t::tv_address())]);
        let connect = gtk::Button::with_label(&t::tv_connect());
        connect.set_sensitive(false);
        let field = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        field.append(&address);
        field.append(&connect);
        let address_box = gtk::Box::new(gtk::Orientation::Vertical, 6);
        address_box.set_margin_top(6);
        address_box.append(&field);
        address_box.append(&widgets::haze(&t::tv_address_hint(), "caption"));
        address_box.set_visible(false);
        body.append(&address_box);

        Self {
            view,
            title,
            done,
            line,
            list,
            network_heading,
            cast_heading,
            cast_list,
            drawn_cast: RefCell::default(),
            searching,
            stages,
            none_line,
            cast_line,
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
        self.title.set_title(&if request.is_some() {
            t::tv_play_on()
        } else {
            t::tv_sheet_title_pair()
        });
        self.line.set_label(&match &request {
            None => t::tv_open_on_tv(),
            Some(r) if r.title.is_empty() => t::tv_episode(&r.episode),
            Some(r) => t::tv_title_episode(&r.title, &r.episode),
        });
        self.none_line.set_label(&if request.is_none() {
            t::tv_none_found_short()
        } else {
            t::tv_none_found_detail()
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
        // Cast TVs, once there is something to cast to them.
        let casts = if request.is_some() { tv.cast_rows() } else { Vec::new() };
        if *self.drawn_cast.borrow() != casts {
            while let Some(child) = self.cast_list.first_child() {
                self.cast_list.remove(&child);
            }
            for row in &casts {
                self.cast_list.append(&cast_row(row));
            }
            self.drawn_cast.replace(casts.clone());
        }
        let any_cast = !casts.is_empty();
        self.cast_list.set_visible(any_cast);
        self.cast_heading.set_visible(any_cast);
        self.network_heading.set_visible(any_cast && !self.drawn.borrow().is_empty());
        self.cast_line.set_visible(request.is_some());
        self.show_searching(tv);
    }

    /// The searching row, while no TV is listed.
    fn show_searching(&self, _tv: &Tv) {
        let empty = self.drawn.borrow().is_empty() && self.drawn_cast.borrow().is_empty();
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
                    &t::tv_none_found_announce(&self.none_line.label()),
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
        .subtitle(super::row_detail(row.kind))
        .use_markup(false)
        .activatable(row.kind != TvRowKind::NeedsUpdate)
        .build();
    line.set_widget_name(&row.key);
    match row.kind {
        TvRowKind::New => {
            let pair = gtk::Label::new(Some(&t::action_pair()));
            pair.add_css_class("heading");
            line.add_suffix(&pair);
            line.upcast_ref::<gtk::Widget>().update_property(&[
                gtk::accessible::Property::Description(&t::tv_row_pairs_with()),
            ]);
        }
        TvRowKind::Paired | TvRowKind::PairedAway if connected => {
            let check = gtk::Image::from_icon_name("object-select-symbolic");
            check.update_property(&[gtk::accessible::Property::Label(
                &t::tv_status_connected(),
            )]);
            line.add_suffix(&check);
        }
        TvRowKind::Paired | TvRowKind::PairedAway => line.add_suffix(&widgets::chevron()),
        TvRowKind::NeedsUpdate => line.set_sensitive(false),
    }
    line
}

/// A row for a TV with Chromecast built in: its name, and that the stream plays through this
/// computer.
fn cast_row(row: &CastRow) -> adw::ActionRow {
    let line = adw::ActionRow::builder()
        .title(&row.name)
        .subtitle(super::cast::route_line(row).unwrap_or_default())
        .use_markup(false)
        .activatable(true)
        .build();
    line.set_widget_name(&row.key);
    line.add_prefix(&gtk::Image::from_icon_name("anistream-cast-symbolic"));
    line.add_suffix(&widgets::chevron());
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
    shown_problem: RefCell<Option<Problem>>,
}

impl Pairing {
    fn new() -> Self {
        let title = adw::WindowTitle::new(&t::tv_sheet_title_pair(), "");
        let (view, header, body) = face(&title);
        let back = gtk::Button::with_label(&t::action_back());
        header.pack_start(&back);
        let pair = gtk::Button::with_label(&t::action_pair());
        pair.add_css_class("suggested-action");
        pair.set_sensitive(false);
        header.pack_end(&pair);
        let spinner = adw::Spinner::new();
        spinner.set_size_request(16, 16);
        // Clear of the sheet's edge, where Pair's own padding would be.
        spinner.set_margin_end(10);
        header.pack_end(&spinner);
        let line = line();
        body.append(&line);
        let code = CodeBoxes::new();
        code.widget.set_margin_top(12);
        code.widget.set_margin_bottom(6);
        body.append(&code.widget);
        let show_new = gtk::Button::with_label(&t::action_new_code());
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
            self.title.set_title(&t::tv_pair_with(&step.tv_name));
            self.line.set_label(&t::tv_type_code(&step.tv_name));
        } else {
            self.title.set_title(&if step.tv_name.is_empty() {
                t::tv_connecting_title()
            } else {
                t::tv_pairing_with(&step.tv_name)
            });
            self.line.set_label(&t::tv_pairing_moment());
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
        self.show_new.set_label(&t::action_new_code());
        if expired {
            self.show_new.add_css_class("suggested-action");
        } else {
            self.show_new.remove_css_class("suggested-action");
        }
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
        // After the clear, which says nothing: Pair lights only over six digits.
        self.pair.set_visible(typing);
        self.pair.set_sensitive(self.code.six().is_some() && !step.working && !expired);
        self.shown_problem.replace(step.problem);
        self.problem.show(
            expired.then(t::tv_code_expired).as_deref(),
            step.problem.map(super::problem_text).as_deref(),
        );
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
        row.update_property(&[gtk::accessible::Property::Label(&t::tv_pairing_code())]);
        let text = gtk::Text::builder()
            .input_purpose(gtk::InputPurpose::Digits)
            .max_length(12)
            .opacity(0.0)
            .can_target(false)
            .build();
        text.update_property(&[gtk::accessible::Property::Label(&t::tv_pairing_code())]);
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
    /// "Skipped opening" and Undo, in the skip's place while an automatic skip can be undone.
    undo_note: gtk::Box,
    skipped: gtk::Label,
    undo: gtk::Button,
    /// Up next on a cast's card: the episode, its count, Cancel and Play now.
    up_next: gtk::Box,
    up_next_cover: gtk::Box,
    up_next_cover_of: Cell<Option<u32>>,
    up_next_line: gtk::Label,
    up_next_episode: gtk::Label,
    up_next_count: gtk::ProgressBar,
    /// The most seconds the count showed for its episode, for how full the bar is.
    up_next_from: RefCell<Option<(String, u32)>>,
    up_next_cancel: gtk::Button,
    up_next_play: gtk::Button,
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
    subtitle_row: adw::ActionRow,
    audio_row: adw::ActionRow,
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
    // The cast's card.
    foot: gtk::Box,
    seek_line: gtk::Box,
    notice: gtk::Label,
    volume_scale: gtk::Scale,
    volume_drag: Cell<Option<f64>>,
    volume_rest: RefCell<Option<glib::SourceId>>,
    volume_held: Cell<Option<(f64, Instant)>>,
    volume_level: Cell<f64>,
    mute: gtk::Button,
    cast_foot: gtk::Box,
    play_here: gtk::Button,
    stop: gtk::Button,
    try_again: gtk::Button,
    choose_another: gtk::Button,
    choose_source: gtk::Button,
    /// The stage the card last drew, for the fade as it starts to play.
    stage: Cell<Option<Stage>>,
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
        let done = gtk::Button::with_label(&t::action_done());
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
        let skip = gtk::Button::with_label(&t::player_skip_opening());
        skip.set_valign(gtk::Align::Center);
        head.append(&skip);
        let undo_note = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        undo_note.set_valign(gtk::Align::Center);
        undo_note.set_visible(false);
        let skipped = widgets::haze("", "caption");
        undo_note.append(&skipped);
        let undo = gtk::Button::with_label(&t::action_undo());
        undo_note.append(&undo);
        head.append(&undo_note);
        now.append(&head);
        // Why a cast ended, under the episode, in the engine's words.
        let notice = line();
        notice.set_visible(false);
        now.append(&notice);

        let seek = gtk::Scale::with_range(gtk::Orientation::Horizontal, 0.0, 1.0, 1.0);
        seek.set_draw_value(false);
        seek.set_hexpand(true);
        seek.set_increments(10.0, 60.0);
        seek.update_property(&[gtk::accessible::Property::Label(&t::player_position())]);
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

        // Up next, near the end of a cast's episode.
        let up_next = gtk::Box::new(gtk::Orientation::Vertical, 10);
        up_next.add_css_class("tv-up-next");
        up_next.set_visible(false);
        let up_next_head = gtk::Box::new(gtk::Orientation::Horizontal, 12);
        let up_next_cover = gtk::Box::new(gtk::Orientation::Horizontal, 0);
        up_next_cover.set_valign(gtk::Align::Center);
        up_next_head.append(&up_next_cover);
        let up_next_text = gtk::Box::new(gtk::Orientation::Vertical, 2);
        up_next_text.set_valign(gtk::Align::Center);
        up_next_text.set_hexpand(true);
        let up_next_line = widgets::haze("", "caption");
        up_next_line.set_xalign(0.0);
        up_next_text.append(&up_next_line);
        let up_next_episode = gtk::Label::new(None);
        up_next_episode.set_xalign(0.0);
        up_next_episode.set_ellipsize(gtk::pango::EllipsizeMode::End);
        up_next_text.append(&up_next_episode);
        up_next_head.append(&up_next_text);
        up_next.append(&up_next_head);
        let up_next_count = gtk::ProgressBar::new();
        up_next_count.add_css_class("tv-up-next-count");
        up_next_count.add_css_class("water");
        up_next.append(&up_next_count);
        let up_next_buttons = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        up_next_buttons.set_homogeneous(true);
        let up_next_cancel = gtk::Button::with_label(&t::action_cancel());
        let up_next_play = gtk::Button::with_label(&t::action_play_now());
        up_next_play.add_css_class("suggested-action");
        up_next_buttons.append(&up_next_cancel);
        up_next_buttons.append(&up_next_play);
        up_next.append(&up_next_buttons);
        now.append(&up_next);

        let back10 = glyph("media-seek-backward-symbolic", &t::player_back_10());
        let play = glyph("media-playback-pause-symbolic", &t::action_pause());
        play.add_css_class("tv-play");
        let forward10 = glyph("media-seek-forward-symbolic", &t::player_forward_10());
        let next = glyph("media-skip-forward-symbolic", &t::player_next_episode());
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
        subtitles.update_property(&[gtk::accessible::Property::Label(&t::player_subtitles())]);
        let subtitle_row = adw::ActionRow::builder().title(t::player_subtitles()).build();
        subtitle_row.add_suffix(&subtitles);
        subtitle_row.set_activatable_widget(Some(&subtitles));
        rows.append(&subtitle_row);
        let audio = menu_button();
        audio.update_property(&[gtk::accessible::Property::Label(&t::player_audio())]);
        let audio_row = adw::ActionRow::builder().title(t::player_audio()).build();
        audio_row.add_suffix(&audio);
        audio_row.set_activatable_widget(Some(&audio));
        rows.append(&audio_row);
        let volume = adw::ActionRow::builder().title(t::tv_volume()).build();
        // "40 of 100" in even digits, as the times and the code.
        volume.add_css_class("numeric");
        let volume_down = glyph("audio-volume-low-symbolic", &t::tv_volume_down());
        let volume_up = glyph("audio-volume-high-symbolic", &t::tv_volume_up());
        volume.add_suffix(&volume_down);
        volume.add_suffix(&volume_up);
        // A cast's TV that takes a level has a scale; one with its own volume in charge has Mute.
        let volume_scale =
            gtk::Scale::with_range(gtk::Orientation::Horizontal, 0.0, 100.0, 1.0);
        volume_scale.set_draw_value(false);
        volume_scale.set_increments(5.0, 10.0);
        volume_scale.set_size_request(180, -1);
        volume_scale.set_valign(gtk::Align::Center);
        volume_scale.set_visible(false);
        volume_scale.update_property(&[gtk::accessible::Property::Label(&t::tv_volume())]);
        volume.add_suffix(&volume_scale);
        let mute = gtk::Button::with_label(&t::tv_cast_mute());
        mute.set_valign(gtk::Align::Center);
        mute.set_visible(false);
        volume.add_suffix(&mute);
        rows.append(&volume);
        now.append(&rows);
        body.append(&now);

        let nothing = line();
        nothing.set_label(&t::tv_after_pairing());
        body.append(&nothing);

        let search = gtk::Entry::builder()
            .placeholder_text(t::tv_search_on_tv())
            .primary_icon_name("system-search-symbolic")
            .margin_top(6)
            .build();
        search.update_property(&[gtk::accessible::Property::Label(&t::tv_search_on_tv())]);
        body.append(&search);
        let problem = ProblemLine::new();
        body.append(&problem.widget);

        let foot = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        foot.set_margin_top(6);
        let pair_again = gtk::Button::with_label(&t::tv_pair_again());
        let another = gtk::Button::with_label(&t::tv_choose_another());
        let disconnect = gtk::Button::with_label(&t::action_disconnect());
        foot.append(&pair_again);
        foot.append(&another);
        foot.append(&disconnect);
        body.append(&foot);

        // A cast's buttons, in the order each ending offers them.
        let cast_foot = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        cast_foot.set_homogeneous(true);
        cast_foot.set_margin_top(6);
        cast_foot.set_visible(false);
        let choose_source = gtk::Button::with_label(&t::player_choose_source());
        let try_again = gtk::Button::with_label(&t::action_try_again());
        let play_here = gtk::Button::with_label(&t::tv_cast_play_here());
        let choose_another = gtk::Button::with_label(&t::tv_choose_another());
        let stop = gtk::Button::with_label(&t::tv_cast_stop());
        for button in [&choose_source, &try_again, &play_here, &choose_another, &stop] {
            cast_foot.append(button);
        }
        body.append(&cast_foot);

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
            undo_note,
            skipped,
            undo,
            up_next,
            up_next_cover,
            up_next_cover_of: Cell::new(None),
            up_next_line,
            up_next_episode,
            up_next_count,
            up_next_from: RefCell::default(),
            up_next_cancel,
            up_next_play,
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
            subtitle_row,
            audio_row,
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
            foot,
            seek_line,
            notice,
            volume_scale,
            volume_drag: Cell::new(None),
            volume_rest: RefCell::default(),
            volume_held: Cell::new(None),
            volume_level: Cell::new(0.0),
            mute,
            cast_foot,
            play_here,
            stop,
            try_again,
            choose_another,
            choose_source,
            stage: Cell::new(None),
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

        // Space plays and pauses, and Left and Right step 10 seconds, as a remote's keys do,
        // while nothing interactive has focus, or the seek line has. A focused button, menu
        // or field keeps its keys, as the player's controls do.
        let keys = gtk::EventControllerKey::new();
        keys.set_propagation_phase(gtk::PropagationPhase::Capture);
        self.view.set_focusable(true);
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
                    !f.is::<gtk::Scale>()
                        && (f.is::<gtk::Button>()
                            || f.is::<gtk::MenuButton>()
                            || f.is::<gtk::ListBoxRow>()
                            || f.is::<gtk::Text>()
                            || f.is::<gtk::Switch>()
                            || f.ancestor(gtk::Popover::static_type()).is_some())
                });
                match tv.upgrade() {
                    Some(tv)
                        if !keeps_keys
                            && ((tv.live() && tv.now().is_some()) || tv.cast_playing()) =>
                    {
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
        if let Some(view) = tv.cast_view() {
            self.update_cast(ctx, &view, came);
            return;
        }
        self.show_link_parts();
        let card = tv.card();
        let live = tv.live();
        let name = card.as_ref().map_or_else(|| "TV".to_owned(), |c| c.tv_name.clone());
        self.title.set_title(&name);
        let line = card.as_ref().and_then(link_line).unwrap_or_else(t::tv_status_connected);
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
            self.draw_now(ctx, now, live, false);
        }
        self.search.set_sensitive(live);
        self.problem.show(None, tv.problem().map(super::problem_text).as_deref());
        let lost = tv.lost();
        self.pair_again.set_visible(lost);
        self.another.set_visible(!lost);
        self.show_playhead(tv);
    }

    fn draw_now(&self, ctx: &Rc<Ctx>, now: &NowCard, live: bool, cast: bool) {
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
        self.episode.set_label(&if now.loading && now.duration.is_none() && !cast {
            t::tv_starting()
        } else {
            episode_line(now)
        });
        match &now.skip {
            Some(skip) => {
                self.skip.set_label(&if skip.label.ends_with("ending") {
                    t::player_skip_ending()
                } else {
                    t::player_skip_opening()
                });
                self.skip.set_visible(true);
            }
            None => self.skip.set_visible(false),
        }
        self.skip.set_sensitive(live);
        let controls = live && !now.loading;
        self.transport.set_sensitive(controls);
        let (icon, tip) = if now.paused {
            ("media-playback-start-symbolic", t::action_play())
        } else {
            ("media-playback-pause-symbolic", t::action_pause())
        };
        self.play.set_icon_name(icon);
        self.play.set_tooltip_text(Some(&tip));
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
        let subtitle = chosen(&now.subtitles, now.subtitles_off, Track::Subtitle);
        let audio = chosen(&now.audio, false, Track::Audio);
        self.subtitles.set_label(&subtitle);
        self.audio.set_label(&audio);
        // A screen reader hears the choice with the row's name: "Subtitles, English · Full".
        self.subtitles.update_property(&[gtk::accessible::Property::Label(
            &t::tv_track_choice(&t::player_subtitles(), &subtitle),
        )]);
        self.audio.update_property(&[gtk::accessible::Property::Label(&t::tv_track_choice(
            &t::player_audio(),
            &audio,
        ))]);
        self.subtitles.set_sensitive(live && !now.subtitles.is_empty());
        self.audio.set_sensitive(live && !now.audio.is_empty());
        self.volume.set_subtitle(
            &now.volume.map(|v| t::tv_volume_of(v.round() as i64)).unwrap_or_default(),
        );
        self.volume.set_sensitive(live);
        if !cast {
            self.rows.set_opacity(if live { 1.0 } else { 0.6 });
        }
    }

    /// The link's own parts back, as a cast's card hid them.
    fn show_link_parts(&self) {
        self.foot.set_visible(true);
        self.search.set_visible(true);
        self.cast_foot.set_visible(false);
        self.notice.set_visible(false);
        self.volume_scale.set_visible(false);
        self.mute.set_visible(false);
        self.volume_down.set_visible(true);
        self.volume_up.set_visible(true);
        self.seek_line.set_visible(true);
        self.transport.set_visible(true);
        self.rows.set_visible(true);
        self.subtitle_row.set_visible(true);
        self.audio_row.set_visible(true);
        self.up_next.set_visible(false);
        self.undo_note.set_visible(false);
        for part in [
            self.seek_line.upcast_ref::<gtk::Widget>(),
            self.transport.upcast_ref(),
            self.rows.upcast_ref(),
            self.position.upcast_ref(),
            self.remaining.upcast_ref(),
        ] {
            part.set_opacity(1.0);
        }
        self.stage.set(None);
    }

    /// The cast's card: where it is in the line above, the episode, the controls once it plays,
    /// and under them the buttons that fit.
    fn update_cast(&self, ctx: &Rc<Ctx>, view: &CastView, came: bool) {
        let tv = &ctx.tv;
        self.title.set_title(&t::tv_play_on());
        let (line, mark) = match view.stage {
            Stage::Starting => (view.starting.clone(), "haze"),
            Stage::Playing => (t::tv_status_playing(&view.name), "water"),
            Stage::Failed => (t::tv_cast_couldnt_start(&view.name), "vermilion"),
            Stage::Lost => (t::tv_cast_stopped(), "vermilion"),
        };
        let faded = self.shown_status.borrow().1 != mark;
        if *self.shown_status.borrow() != (line.clone(), mark) {
            while let Some(child) = self.status.first_child() {
                self.status.remove(&child);
            }
            let standing = widgets::standing(&line, mark);
            self.status.append(&standing);
            // The line crossfades as the cast moves on; only its seconds change in place.
            if faded && !came {
                crate::motion::fade_in(&standing);
            }
            if faded && view.stage.ended() {
                standing.announce(&line, gtk::AccessibleAnnouncementPriority::High);
            }
            self.shown_status.replace((line, mark));
        }
        if came {
            self.stage.set(None);
        }
        self.foot.set_visible(false);
        self.search.set_visible(false);
        self.nothing.set_visible(false);
        self.problem.show(None, tv.problem().map(super::problem_text).as_deref());

        let ended = view.stage.ended();
        let playing = view.stage == Stage::Playing;
        let notice = view.notice.as_ref().map(crate::say::msg);
        self.notice.set_visible(notice.is_some());
        self.notice.set_label(notice.as_deref().unwrap_or_default());
        // What went wrong is the standing line's heading and this text; nothing else moves.
        self.seek_line.set_visible(view.stage != Stage::Failed);
        self.transport.set_visible(!ended);
        self.rows.set_visible(!ended);

        self.now.set_visible(view.now.is_some());
        if let Some(now) = &view.now {
            self.draw_now(ctx, now, playing, true);
            if ended || view.undo.is_some() {
                self.skip.set_visible(false);
            }
            // Subtitles once the TV has said what it has, never a guess while it starts; audio
            // only while the TV can be given a choice, since a change loads the stream again.
            self.subtitle_row.set_visible(playing || !now.subtitles.is_empty());
            self.audio_row.set_visible(!now.audio.is_empty());
            self.draw_up_next(ctx, view.up_next.as_ref(), now);
        }
        self.undo_note.set_visible(view.undo.is_some());
        if let Some(undo) = &view.undo {
            self.skipped.set_label(&super::cast::skipped_line(undo));
        }
        // Dimmed while it starts, and in full once the TV shows a picture.
        let was = self.stage.replace(Some(view.stage));
        if !ended {
            let parts = [
                self.seek_line.upcast_ref::<gtk::Widget>(),
                self.transport.upcast_ref(),
                self.rows.upcast_ref(),
            ];
            for part in parts {
                if playing && was.is_some_and(|w| w != Stage::Playing) {
                    crate::motion::quick(part, 0.4, 1.0, crate::motion::opacity(part)).play();
                } else if !playing {
                    part.set_opacity(0.4);
                } else if was.is_none() {
                    part.set_opacity(1.0);
                }
            }
        }
        self.position.set_opacity(if playing || ended { 1.0 } else { 0.0 });
        self.remaining.set_opacity(if playing { 1.0 } else { 0.0 });

        self.draw_cast_volume(view, playing);
        self.draw_cast_buttons(view);
        self.show_playhead(tv);
    }

    /// Up next while the engine sends it: the line with its count, the episode, and the count as
    /// a bar that fills as it runs down.
    fn draw_up_next(&self, ctx: &Rc<Ctx>, next: Option<&CastUpNext>, now: &NowCard) {
        self.up_next.set_visible(next.is_some());
        let Some(next) = next else {
            self.up_next_from.replace(None);
            return;
        };
        let id = now.id.get();
        if self.up_next_cover_of.replace(Some(id)) != Some(id) {
            while let Some(child) = self.up_next_cover.first_child() {
                self.up_next_cover.remove(&child);
            }
            if let Some(entry) = ctx.hub.entry(now.id) {
                self.up_next_cover.append(&widgets::cover(&ctx.hub, &entry, 40, 60));
            }
        }
        self.up_next_line.set_label(&super::cast::up_next_line(next));
        self.up_next_episode.set_label(&super::cast::up_next_episode(next));
        self.up_next_count.set_visible(next.seconds_left.is_some());
        if let Some(left) = next.seconds_left {
            let mut from = self.up_next_from.borrow_mut();
            if from.as_ref().is_none_or(|(number, most)| *number != next.number || left > *most)
            {
                from.replace((next.number.clone(), left.max(1)));
            }
            let most = from.as_ref().map_or(1, |(_, most)| *most);
            self.up_next_count.set_fraction(1.0 - f64::from(left) / f64::from(most));
        }
    }

    /// TV volume: a scale for a TV that takes a level, Mute for one whose own volume is in
    /// charge.
    fn draw_cast_volume(&self, view: &CastView, playing: bool) {
        let level = matches!(view.volume, VolumeControl::Level);
        self.volume_down.set_visible(false);
        self.volume_up.set_visible(false);
        self.volume_scale.set_visible(level);
        self.mute.set_visible(!level);
        self.mute.set_label(&if view.muted { t::tv_cast_unmute() } else { t::tv_cast_mute() });
        self.mute.set_sensitive(playing);
        self.volume_scale.set_sensitive(playing);
        let reported = view.now.as_ref().and_then(|n| n.volume);
        if let Some(volume) = reported {
            self.volume_level.set(volume);
        }
        if self.volume_held.get().is_some_and(|(_, at)| at.elapsed() > VOLUME_HOLD) {
            self.volume_held.set(None);
        }
        if self.volume_drag.get().is_none() {
            let shown = self.volume_held.get().map(|h| h.0).or(reported).unwrap_or(0.0);
            self.volume_scale.set_value(shown);
        }
        self.volume.set_subtitle(&if level {
            String::new()
        } else {
            t::tv_cast_volume_on_tv()
        });
    }

    /// Play here and Stop casting while it runs; after an ending, what the engine's reason
    /// offers.
    fn draw_cast_buttons(&self, view: &CastView) {
        self.cast_foot.set_visible(true);
        let offered = |offer| view.offers.contains(&offer);
        self.choose_source.set_visible(offered(Offer::ChooseSource));
        self.try_again.set_visible(offered(Offer::TryAgain));
        self.play_here.set_visible(!view.stage.ended() || offered(Offer::PlayHere));
        self.choose_another.set_visible(offered(Offer::ChooseAnother));
        self.stop.set_visible(!view.stage.ended());
    }

    /// The scale moved by the viewer: held until it rests, then sent as a step.
    fn volume_dragged(&self, value: f64) {
        self.volume_drag.set(Some(value));
        if let Some(id) = self.volume_rest.take() {
            id.remove();
        }
    }

    fn volume_rested(&self, tv: &Tv) {
        self.volume_rest.take();
        let Some(target) = self.volume_drag.take() else { return };
        let step = target - self.volume_level.get();
        self.volume_held.set(Some((target, Instant::now())));
        if step.abs() >= 1.0 {
            tv.cast_send(RemoteCommand::Volume { step });
        }
    }

    /// Moved by the viewer: the line follows, and once the drag rests the TV is told.
    fn dragged(&self, value: f64) {
        self.dragging.set(Some(value));
        self.position.set_label(&say::clock(value));
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
        let casting = tv.cast_view();
        let Some(now) = casting.as_ref().map_or_else(|| tv.now(), |view| view.now.clone())
        else {
            return;
        };
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
            .or_else(|| if casting.is_some() { tv.cast_playhead() } else { tv.playhead() })
            .unwrap_or(now.position);
        // Before the TV knows the length, the bar waits empty rather than reading full.
        if self.dragging.get().is_none() {
            self.seek.set_value(if now.duration.is_some() { position } else { 0.0 });
        }
        self.position.set_label(&say::clock(position));
        self.remaining.set_label(
            &now.duration
                .map(|d| format!("\u{2212}{}", say::clock((d - position).max(0.0))))
                .unwrap_or_default(),
        );
        self.seek
            .update_property(&[gtk::accessible::Property::ValueText(&say::clock(position))]);
    }
}

/// What the card says of the link: its name for the phase, or nothing when it is up.
fn link_line(card: &anistream_engine::remote::sender::RemoteCard) -> Option<String> {
    let name = &card.tv_name;
    match card.phase {
        LinkPhase::Connected => None,
        LinkPhase::Connecting => Some(t::tv_status_connecting(name)),
        LinkPhase::Retrying => Some(t::tv_status_reconnecting(name)),
        LinkPhase::Unpaired => Some(super::problem_text(Problem::Unpaired)),
        LinkPhase::KeyChanged => Some(super::problem_text(Problem::KeyChanged)),
    }
}

/// The episode on the TV's card: "Episode 5", or "Episode 5 · Title" when the TV knows its title.
fn episode_line(now: &NowCard) -> String {
    match now.episode_line.split_once(" \u{b7} ") {
        Some((_, title)) => t::tv_episode_titled(&now.episode, title),
        None => t::tv_episode(&now.episode),
    }
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
        return t::value_off();
    }
    tracks.iter().find(|t| t.selected).map_or_else(t::value_none, |t| track_label(t, kind))
}

fn track_menu(tracks: &[TrackInfo], kind: Track) -> gio::Menu {
    let (action, menu) = match kind {
        Track::Subtitle => ("tv.subtitle", gio::Menu::new()),
        Track::Audio => ("tv.audio", gio::Menu::new()),
    };
    if kind == Track::Subtitle {
        menu.append_item(&widgets::menu_item(&t::value_off(), action, "off"));
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
