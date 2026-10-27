//! Starting a watch party, or joining one from an invite: the server, with the suggested public
//! one and who runs it, the room the engine named, and the name the room sees. Start Party joins
//! and stays up, saying so, until the room answers; a server that can't be reached or refuses
//! says why here, and Try Again keeps what was typed. Once in the room the dialog goes, and the
//! party lives on the player's people button. A dialog, which a narrow window shows as a bottom
//! sheet.

use std::{
    cell::{Cell, RefCell},
    rc::{Rc, Weak},
};

use adw::prelude::*;
use anistream_core::ids::AnilistId;
use anistream_i18n::t;
use anistream_model::{command::Command, event::Event, party::PartyInvite};
use gtk::gio;

use crate::{
    widgets,
    window::{Ctx, Size},
};

/// What the party is started for.
#[derive(Debug, Clone)]
pub enum For {
    /// From a title's More menu: this episode plays with the room.
    Episode(AnilistId, String),
    /// From the player's people button: what plays now plays with the room.
    Playing,
    /// From an invite link: its room, on its server, with its episode.
    Invite(PartyInvite),
}

struct Form {
    ctx: Rc<Ctx>,
    what: For,
    dialog: adw::Dialog,
    start: gtk::Button,
    episode: gtk::Label,
    server: adw::EntryRow,
    recent: gtk::MenuButton,
    hint: gtk::Label,
    use_suggested: gtk::Button,
    room: adw::EntryRow,
    name: adw::EntryRow,
    foot: gtk::Stack,
    spinner: adw::Spinner,
    mark: gtk::Image,
    status: gtk::Label,
    joining: Cell<bool>,
    filled: Cell<bool>,
    /// Set while the dialog fills its own fields, so the edits are not the viewer's.
    filling: Cell<bool>,
}

/// What the foot says before the form can be filled in.
fn getting_ready() -> String {
    t::party_getting_ready()
}

/// What the foot says while the device is offline.
fn offline() -> String {
    t::party_needs_connection()
}

thread_local! {
    /// The dialog that is up, for an invite opened while it is and for a shot's typing.
    static SHOWING: RefCell<Weak<Form>> = const { RefCell::new(Weak::new()) };
}

/// Whether the dialog is up and waiting on the room: it says why a party ended itself.
pub fn joining() -> bool {
    SHOWING.with(|s| s.borrow().upgrade().is_some_and(|f| f.joining.get()))
}

/// Open the dialog for `what`, in place of one already up.
pub fn open(ctx: &Rc<Ctx>, what: For) {
    if let Some(old) = SHOWING.with(|s| s.borrow().upgrade()) {
        old.dialog.force_close();
    }
    let invite = matches!(what, For::Invite(_));
    let header = adw::HeaderBar::new();
    header.set_show_end_title_buttons(false);
    header.set_show_start_title_buttons(false);
    let cancel = gtk::Button::with_label(&t::action_cancel());
    header.pack_start(&cancel);
    let start = gtk::Button::with_label(&if invite { t::party_join() } else { t::party_start() });
    start.add_css_class("suggested-action");
    start.set_sensitive(false);
    header.pack_end(&start);

    let column = gtk::Box::new(gtk::Orientation::Vertical, 12);
    column.set_margin_top(12);
    column.set_margin_bottom(24);
    column.set_margin_start(18);
    column.set_margin_end(18);
    let episode = gtk::Label::new(None);
    episode.add_css_class("heading");
    episode.set_xalign(0.0);
    episode.set_wrap(true);
    column.append(&episode);

    let server = adw::EntryRow::builder().title(t::party_server()).build();
    server.set_input_purpose(gtk::InputPurpose::Url);
    let recent = gtk::MenuButton::builder()
        .icon_name("document-open-recent-symbolic")
        .tooltip_text(t::party_recent_servers_heading())
        .valign(gtk::Align::Center)
        .visible(false)
        .build();
    recent.add_css_class("flat");
    server.add_suffix(&recent);
    let hint = widgets::haze("", "caption");
    hint.set_hexpand(true);
    let use_suggested = gtk::Button::with_label(&t::party_use_it());
    use_suggested.add_css_class("flat");
    use_suggested.add_css_class("hint-button");
    use_suggested.set_valign(gtk::Align::Start);
    let room = adw::EntryRow::builder().title(t::party_room()).build();
    let name = adw::EntryRow::builder().title(t::party_your_name()).build();
    name.set_max_length(40);
    if let For::Invite(link) = &what {
        // An invite names the room: it is shown, not asked, with whose server it is.
        let place = widgets::boxed_list();
        for (title, value) in [(t::party_room(), &link.room), (t::party_server(), &link.server)] {
            let row = adw::ActionRow::builder().title(title).subtitle(value).build();
            row.add_css_class("property");
            row.set_subtitle_selectable(true);
            place.append(&row);
        }
        column.append(&place);
        column.append(&hint);
        let names = widgets::boxed_list();
        names.append(&name);
        column.append(&names);
    } else {
        let servers = widgets::boxed_list();
        servers.append(&server);
        column.append(&servers);
        let hint_line = gtk::Box::new(gtk::Orientation::Horizontal, 6);
        hint_line.set_margin_top(-6);
        hint_line.append(&hint);
        hint_line.append(&use_suggested);
        column.append(&hint_line);
        let rest = widgets::boxed_list();
        rest.append(&room);
        rest.append(&name);
        column.append(&rest);
        let note = widgets::haze(&t::party_room_hint(), "caption");
        note.set_margin_top(-6);
        column.append(&note);
    }

    // The privacy note and the joining or error line share one place: the line stands in for
    // the note once Start is pressed, so the dialog keeps its height throughout.
    let privacy = widgets::haze(
        &format!("{} {}", t::party_privacy(), t::legal_hosts_no_content()),
        "caption",
    );
    let spinner = adw::Spinner::new();
    spinner.set_size_request(16, 16);
    spinner.set_valign(gtk::Align::Start);
    let mark = gtk::Image::from_icon_name("dialog-error-symbolic");
    mark.add_css_class("error");
    mark.set_valign(gtk::Align::Start);
    let status = gtk::Label::new(None);
    status.set_xalign(0.0);
    status.set_wrap(true);
    status.set_wrap_mode(gtk::pango::WrapMode::WordChar);
    status.set_hexpand(true);
    status.add_css_class("caption");
    status.update_property(&[gtk::accessible::Property::Label(&t::party_status())]);
    let line = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    line.set_valign(gtk::Align::Start);
    line.append(&spinner);
    line.append(&mark);
    line.append(&status);
    let foot = gtk::Stack::new();
    foot.set_transition_type(gtk::StackTransitionType::Crossfade);
    foot.set_transition_duration(150);
    foot.set_vhomogeneous(true);
    foot.set_hhomogeneous(true);
    foot.add_named(&privacy, Some("privacy"));
    foot.add_named(&line, Some("status"));
    foot.set_margin_top(6);
    column.append(&foot);

    if ctx.size() != Size::Narrow {
        column.set_size_request(440, -1);
    }
    let scroll = gtk::ScrolledWindow::builder()
        .hscrollbar_policy(gtk::PolicyType::Never)
        .propagate_natural_height(true)
        .max_content_height(640)
        .child(&column)
        .build();
    let view = adw::ToolbarView::new();
    view.add_top_bar(&header);
    view.set_content(Some(&scroll));
    let dialog = adw::Dialog::builder()
        .title(if invite { t::party_join_title() } else { t::party_title() })
        .follows_content_size(true)
        .child(&view)
        .build();
    dialog.set_default_widget(Some(&start));

    let form = Rc::new(Form {
        ctx: Rc::clone(ctx),
        what,
        dialog: dialog.clone(),
        start: start.clone(),
        episode,
        server: server.clone(),
        recent,
        hint,
        use_suggested: use_suggested.clone(),
        room: room.clone(),
        name: name.clone(),
        foot,
        spinner,
        mark,
        status,
        joining: Cell::new(false),
        filled: Cell::new(false),
        filling: Cell::new(false),
    });
    form.show_episode();
    for field in [&server, &room, &name] {
        field.set_sensitive(false);
        field.connect_changed({
            let weak = Rc::downgrade(&form);
            move |_| {
                if let Some(form) = weak.upgrade()
                    && !form.filling.get()
                {
                    form.update();
                }
            }
        });
        // Return in a field starts, as the button would.
        field.connect_entry_activated({
            let weak = Rc::downgrade(&form);
            move |_| {
                if let Some(form) = weak.upgrade()
                    && form.start.is_sensitive()
                {
                    form.join();
                }
            }
        });
    }
    let recent_action = gio::SimpleAction::new("recent", Some(gtk::glib::VariantTy::STRING));
    recent_action.connect_activate({
        let weak = Rc::downgrade(&form);
        move |_, arg| {
            if let (Some(form), Some(text)) = (weak.upgrade(), arg.and_then(|a| a.str())) {
                form.server.set_text(text);
                form.server.set_position(-1);
            }
        }
    });
    let actions = gio::SimpleActionGroup::new();
    actions.add_action(&recent_action);
    dialog.insert_action_group("party-form", Some(&actions));
    use_suggested.connect_clicked({
        let weak = Rc::downgrade(&form);
        move |_| {
            let Some(form) = weak.upgrade() else { return };
            let suggested = form
                .ctx
                .hub
                .known
                .borrow()
                .party_form
                .as_ref()
                .map(|f| f.suggested_server.clone());
            if let Some(suggested) = suggested {
                form.server.set_text(&suggested);
                form.server.set_position(-1);
            }
        }
    });
    cancel.connect_clicked({
        let dialog = dialog.clone();
        move |_| {
            dialog.close();
        }
    });
    start.connect_clicked({
        let weak = Rc::downgrade(&form);
        move |_| {
            if let Some(form) = weak.upgrade() {
                form.join();
            }
        }
    });
    let listening = ctx.hub.listen({
        let weak = Rc::downgrade(&form);
        move |event| {
            if let Some(form) = weak.upgrade() {
                form.heard(event);
            }
        }
    });
    dialog.connect_closed({
        let weak = Rc::downgrade(&form);
        let hub = Rc::downgrade(&ctx.hub);
        move |_| {
            if let Some(hub) = hub.upgrade() {
                hub.forget(listening);
            }
            let Some(form) = weak.upgrade() else { return };
            // Cancel while the room is still answering: no party behind the viewer's back.
            if form.joining.replace(false) {
                form.ctx.hub.send(Command::PartyLeave);
            }
        }
    });
    widgets::keep(&dialog, Rc::clone(&form));
    SHOWING.with(|s| s.replace(Rc::downgrade(&form)));

    ctx.hub.known.borrow_mut().party_form = None;
    form.update();
    ctx.hub.send(Command::LoadPartyForm);
    dialog.present(Some(&ctx.window));
}

/// For a shot: the server typed into the dialog that is up, then Start Party, once the form
/// has filled.
#[cfg(feature = "fixtures")]
pub fn rehearse(server: Option<String>, start: bool) {
    use std::time::Duration;
    gtk::glib::spawn_future_local(async move {
        for _ in 0..50 {
            let Some(form) = SHOWING.with(|s| s.borrow().upgrade()) else { return };
            if form.filled.get() {
                if let Some(server) = &server {
                    form.server.set_text(server);
                }
                form.update();
                if start && form.start.is_sensitive() {
                    form.join();
                }
                return;
            }
            gtk::glib::timeout_future(Duration::from_millis(100)).await;
        }
    });
}

impl Form {
    /// The room and server joined: an invite's own, or what is typed.
    fn place(&self) -> (String, String) {
        match &self.what {
            For::Invite(link) => (link.server.clone(), link.room.clone()),
            _ => (self.server.text().trim().to_owned(), self.room.text().trim().to_owned()),
        }
    }

    fn show_episode(&self) {
        let line = match &self.what {
            For::Episode(id, episode) => super::episode_of(&self.ctx, *id, episode),
            For::Invite(link) => super::episode_of(&self.ctx, link.id, &link.episode),
            For::Playing => t::party_form_nothing(),
        };
        self.episode.set_label(&line);
    }

    fn fill(&self) {
        let Some(form) = self.ctx.hub.known.borrow().party_form.clone() else { return };
        if self.filled.replace(true) {
            return;
        }
        self.filling.set(true);
        self.server.set_text(form.recent_servers.first().map_or("", String::as_str));
        self.room.set_text(&form.room);
        self.name.set_text(&form.name);
        self.filling.set(false);
        if !form.recent_servers.is_empty() {
            let menu = gio::Menu::new();
            for server in &form.recent_servers {
                menu.append_item(&widgets::menu_item(server, "party-form.recent", server));
            }
            self.recent.set_menu_model(Some(&menu));
            self.recent.set_visible(true);
        }
        for field in [&self.server, &self.room, &self.name] {
            field.set_sensitive(true);
        }
        let first: &adw::EntryRow =
            if matches!(self.what, For::Invite(_)) { &self.name } else { &self.server };
        first.grab_focus();
        first.set_position(-1);
    }

    /// Whether Start can be pressed, and the line under the server.
    fn update(&self) {
        let known = self.ctx.hub.known.borrow();
        let form = known.party_form.as_ref();
        let (server, room) = self.place();
        let problem = super::server_problem(&server);
        if let Some(form) = form {
            let line = match (&self.what, &problem) {
                (For::Invite(_), _) if super::is_suggested(&server, form) => {
                    super::server_hint(&server, form)
                }
                (For::Invite(_), _) => t::party_server_invited(),
                (_, Some(problem)) => problem.clone(),
                _ => super::server_hint(&server, form),
            };
            self.hint.set_label(&line);
            // Faded rather than removed, so the line keeps its place.
            let suggested = super::is_suggested(&server, form);
            self.use_suggested.set_opacity(if suggested { 0.0 } else { 1.0 });
            self.use_suggested.set_can_target(!suggested);
            self.use_suggested.set_can_focus(!suggested);
            self.use_suggested.set_sensitive(!self.joining.get());
            self.use_suggested
                .set_tooltip_text(Some(&t::party_use_server(&form.suggested_server)));
        } else {
            self.use_suggested.set_opacity(0.0);
            self.use_suggested.set_can_target(false);
        }
        if problem.is_some() {
            self.server.add_css_class("error");
        } else {
            self.server.remove_css_class("error");
        }
        if !self.joining.get() {
            if known.offline {
                self.say(&offline(), false, true);
            } else if form.is_none() {
                self.say(&getting_ready(), true, false);
            } else if [getting_ready(), offline()].contains(&self.status.label().to_string()) {
                self.say("", false, false);
            }
        }
        let name = self.name.text();
        self.start.set_sensitive(
            form.is_some()
                && !self.joining.get()
                && !known.offline
                && !server.is_empty()
                && problem.is_none()
                && !room.is_empty()
                && !name.trim().is_empty(),
        );
    }

    /// The line in the foot: `text` with a spinner while `busy`, the error mark for a
    /// `problem`, or the privacy note back when there is nothing to say.
    fn say(&self, text: &str, busy: bool, problem: bool) {
        self.status.set_label(text);
        self.spinner.set_visible(busy);
        // Only the mark is vermilion: the words stay as readable as the rest.
        self.mark.set_visible(problem);
        self.foot.set_visible_child_name(if text.is_empty() { "privacy" } else { "status" });
    }

    fn join(&self) {
        let (server, room) = self.place();
        let name = self.name.text().trim().to_owned();
        self.joining.set(true);
        for field in [&self.server, &self.room, &self.name] {
            field.set_sensitive(false);
        }
        self.say(&t::party_joining(&room, &server), true, false);
        self.update();
        // The room first, and the episode once in it: a server that can't be reached leaves
        // nothing playing behind the dialog.
        self.ctx.hub.send(Command::PartyJoin { server, room, name: Some(name) });
    }

    fn heard(&self, event: &Event) {
        match event {
            Event::PartyForm(_) => self.fill(),
            Event::PartyJoined(_) if self.joining.replace(false) => {
                // In the room: the episode plays with it, or says it has no source here.
                let play = match &self.what {
                    For::Episode(id, episode) => Some((*id, episode.clone())),
                    For::Invite(link) => Some((link.id, link.episode.clone())),
                    For::Playing => None,
                };
                if let Some((id, episode)) = play {
                    self.ctx.hub.send(Command::PlayParty {
                        id,
                        episode,
                        server: None,
                        room: None,
                        name: None,
                    });
                }
                self.dialog.close();
                return;
            }
            Event::PartyLeft { reason } if self.joining.replace(false) => {
                for field in [&self.server, &self.room, &self.name] {
                    field.set_sensitive(true);
                }
                self.start.set_label(&t::action_try_again());
                let why = reason
                    .as_ref()
                    .map_or_else(t::party_ended_early, crate::say::msg);
                self.say(&why, false, true);
            }
            Event::Detail(_) | Event::Content { .. } => self.show_episode(),
            Event::Connectivity { .. } => {}
            _ => return,
        }
        self.update();
    }
}
