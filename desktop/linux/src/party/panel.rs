//! The party, from the player's people button: how the connection stands, who is in the room
//! and who is ready, this member's own ready switch, the invite link to copy, the server and
//! room as text for someone joining with Syncplay itself, and Leave Party. It follows the engine
//! while it is open, and each part keeps its place as the room changes.

use std::{
    cell::Cell,
    rc::{Rc, Weak},
    time::Duration,
};

use adw::prelude::*;
use anistream_model::{
    command::Command,
    event::Event,
    party::{PartyConnection, PartyMember},
};
use gtk::glib;

use crate::{widgets, window::Ctx};

/// The widest a caption runs in the panel before it wraps.
const MAX_CHARS: i32 = 40;

pub struct Panel {
    me: Weak<Self>,
    ctx: Rc<Ctx>,
    pub popover: gtk::Popover,
    connection: gtk::Label,
    reconnecting: adw::Spinner,
    members: gtk::Box,
    ready_line: gtk::Label,
    ready: gtk::Switch,
    copy: gtk::Button,
    copy_label: gtk::Label,
    invite_note: gtk::Label,
    place: gtk::Label,
    motd: gtk::Label,
    leave: gtk::Button,
    /// Set while the switch follows the engine, so its change is not the viewer's.
    from_engine: Cell<bool>,
    leaving: Cell<bool>,
    copied: Cell<u32>,
}

impl Panel {
    pub fn new(ctx: &Rc<Ctx>) -> Rc<Self> {
        let body = gtk::Box::new(gtk::Orientation::Vertical, 12);
        body.set_width_request(320);
        body.set_margin_top(6);
        body.set_margin_bottom(6);
        body.set_margin_start(6);
        body.set_margin_end(6);

        let head = gtk::Box::new(gtk::Orientation::Vertical, 2);
        let title = gtk::Label::new(Some("Watch Party"));
        title.add_css_class("heading");
        title.set_xalign(0.0);
        head.append(&title);
        let connection = widgets::haze("", "caption");
        let reconnecting = adw::Spinner::new();
        reconnecting.set_size_request(12, 12);
        reconnecting.set_visible(false);
        let status = gtk::Box::new(gtk::Orientation::Horizontal, 6);
        status.append(&connection);
        status.append(&reconnecting);
        head.append(&status);
        body.append(&head);

        let members = gtk::Box::new(gtk::Orientation::Vertical, 2);
        members.update_property(&[gtk::accessible::Property::Label("Members")]);
        body.append(&members);
        let ready_line = widgets::haze("", "caption");
        ready_line.set_margin_top(-6);
        body.append(&ready_line);

        let ready = gtk::Switch::new();
        ready.set_valign(gtk::Align::Center);
        let ready_label = gtk::Label::new(Some("I'm Ready"));
        ready_label.set_xalign(0.0);
        ready_label.set_hexpand(true);
        ready_label.set_mnemonic_widget(Some(&ready));
        ready.update_property(&[gtk::accessible::Property::Label("I'm ready")]);
        let ready_row = gtk::Box::new(gtk::Orientation::Horizontal, 12);
        ready_row.append(&ready_label);
        ready_row.append(&ready);
        body.append(&ready_row);

        body.append(&gtk::Separator::new(gtk::Orientation::Horizontal));

        let copy_label = gtk::Label::new(Some("Copy Invite Link"));
        let copy_line = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        copy_line.set_halign(gtk::Align::Center);
        copy_line.append(&gtk::Image::from_icon_name("edit-copy-symbolic"));
        copy_line.append(&copy_label);
        let copy = gtk::Button::builder().child(&copy_line).build();
        let invite_note = widgets::haze("", "caption");
        invite_note.set_margin_top(-6);
        let invite = gtk::Box::new(gtk::Orientation::Vertical, 6);
        invite.append(&copy);
        invite.append(&invite_note);
        body.append(&invite);

        let plain = gtk::Box::new(gtk::Orientation::Vertical, 2);
        let plain_title = gtk::Label::new(Some("For Syncplay's own app"));
        plain_title.add_css_class("caption-heading");
        plain_title.set_xalign(0.0);
        plain.append(&plain_title);
        let place = widgets::haze("", "caption");
        place.set_selectable(true);
        place.set_wrap_mode(gtk::pango::WrapMode::WordChar);
        plain.append(&place);
        body.append(&plain);
        let motd = widgets::haze("", "caption");
        motd.set_visible(false);
        body.append(&motd);

        let leave = gtk::Button::with_label("Leave Party");
        body.append(&leave);

        // A wrapping label asks for its whole line: held to the panel's width, so the popover
        // stays 320 wide and fits a narrow window.
        for label in [&connection, &ready_line, &invite_note, &place, &motd] {
            label.set_max_width_chars(MAX_CHARS);
        }
        let popover =
            gtk::Popover::builder().child(&body).position(gtk::PositionType::Bottom).build();
        let panel = Rc::new_cyclic(|me| Self {
            me: me.clone(),
            ctx: Rc::clone(ctx),
            popover: popover.clone(),
            connection,
            reconnecting,
            members,
            ready_line,
            ready: ready.clone(),
            copy: copy.clone(),
            copy_label,
            invite_note,
            place,
            motd,
            leave: leave.clone(),
            from_engine: Cell::new(false),
            leaving: Cell::new(false),
            copied: Cell::new(0),
        });
        ready.connect_active_notify({
            let weak = Rc::downgrade(&panel);
            move |switch| {
                if let Some(panel) = weak.upgrade()
                    && !panel.from_engine.get()
                {
                    panel.ctx.hub.send(Command::PartyReady { ready: switch.is_active() });
                }
            }
        });
        copy.connect_clicked({
            let weak = Rc::downgrade(&panel);
            move |_| {
                if let Some(panel) = weak.upgrade() {
                    panel.copy_invite();
                }
            }
        });
        leave.connect_clicked({
            let weak = Rc::downgrade(&panel);
            move |_| {
                let Some(panel) = weak.upgrade() else { return };
                panel.leaving.set(true);
                panel.leave.set_label("Leaving…");
                panel.show();
                panel.ctx.hub.send(Command::PartyLeave);
            }
        });
        popover.connect_show({
            let weak = Rc::downgrade(&panel);
            move |_| {
                if let Some(panel) = weak.upgrade() {
                    panel.leaving.set(false);
                    panel.leave.set_label("Leave Party");
                    panel.show();
                }
            }
        });
        ctx.hub.listen({
            let weak = Rc::downgrade(&panel);
            move |event| {
                let Some(panel) = weak.upgrade() else { return };
                if !panel.popover.is_visible() {
                    return;
                }
                match event {
                    Event::PartyLeft { .. } => panel.popover.popdown(),
                    Event::PartyJoined(_) | Event::PartyState(_) | Event::PartyMembers(_) => {
                        panel.show()
                    }
                    _ => {}
                }
            }
        });
        panel
    }

    /// The party as the engine last said it.
    fn show(&self) {
        let known = self.ctx.hub.known.borrow();
        let Some(party) = known.party.as_ref() else { return };
        let lost = matches!(party.connection, PartyConnection::Reconnecting { .. });
        let leaving = self.leaving.get();
        self.connection.set_label(&super::connection_line(party));
        self.reconnecting.set_visible(lost);

        while let Some(row) = self.members.first_child() {
            self.members.remove(&row);
        }
        match known.party_members.as_deref() {
            Some(members) => {
                for member in members {
                    self.members.append(&row(member));
                }
                self.ready_line.set_label(&super::ready_line(members));
                self.ready_line.set_visible(true);
            }
            None => {
                // Before the room lists its members: one row's height, waiting.
                let wait = gtk::Box::new(gtk::Orientation::Horizontal, 10);
                wait.set_size_request(-1, 40);
                let spinner = adw::Spinner::new();
                spinner.set_size_request(16, 16);
                wait.append(&spinner);
                wait.append(&widgets::haze("Waiting for the room's members", ""));
                self.members.append(&wait);
                self.ready_line.set_visible(false);
            }
        }

        self.from_engine.set(true);
        self.ready.set_active(party.ready);
        self.from_engine.set(false);
        self.ready.set_sensitive(!lost && !leaving);

        self.copy.set_sensitive(party.invite.is_some() && !leaving);
        self.invite_note.set_label(if party.invite.is_some() {
            "It names the room and the episode. Each friend plays it from their own sources."
        } else {
            "The invite link comes once an episode plays."
        });
        self.place.set_label(&format!("Server {}\nRoom {}", party.server, party.room));
        let motd = party.motd.as_deref().filter(|m| !m.trim().is_empty());
        self.motd.set_label(motd.unwrap_or_default());
        self.motd.set_visible(motd.is_some());
        self.leave.set_sensitive(!leaving);
    }

    fn copy_invite(&self) {
        let Some(link) =
            self.ctx.hub.known.borrow().party.as_ref().and_then(|p| p.invite.clone())
        else {
            return;
        };
        self.copy.clipboard().set_text(&link);
        let mine = self.copied.get() + 1;
        self.copied.set(mine);
        self.copy_label.set_label("Copied");
        let me = self.me.clone();
        // Back after 2 s, unless copied again since.
        glib::timeout_add_local_once(Duration::from_secs(2), move || {
            if let Some(panel) = me.upgrade()
                && panel.copied.get() == mine
            {
                panel.copy_label.set_label("Copy Invite Link");
            }
        });
    }
}

/// One member: a water dot when ready, a ring when not, their name and what differs about
/// their file, and the state in words.
fn row(member: &PartyMember) -> gtk::Box {
    let mark = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    mark.add_css_class("mark");
    mark.add_css_class(match member.ready {
        Some(true) => "water",
        Some(false) => "ring",
        None => "haze",
    });
    mark.set_valign(gtk::Align::Center);
    mark.set_size_request(8, 8);
    let names = gtk::Box::new(gtk::Orientation::Vertical, 0);
    names.set_valign(gtk::Align::Center);
    names.set_hexpand(true);
    let name = gtk::Label::new(Some(&super::member_name(member)));
    name.set_xalign(0.0);
    name.set_ellipsize(gtk::pango::EllipsizeMode::End);
    names.append(&name);
    if let Some(file) = super::member_file(member) {
        let file = widgets::haze(file, "caption");
        file.set_max_width_chars(MAX_CHARS);
        names.append(&file);
    }
    let state = gtk::Label::new(Some(super::member_state(member)));
    state.add_css_class("caption");
    if member.ready != Some(true) {
        state.add_css_class("haze");
    }
    state.set_valign(gtk::Align::Center);
    let line = gtk::Box::new(gtk::Orientation::Horizontal, 10);
    line.set_size_request(-1, 40);
    line.append(&mark);
    line.append(&names);
    line.append(&state);
    line.set_accessible_role(gtk::AccessibleRole::Group);
    line.update_property(&[gtk::accessible::Property::Label(&format!(
        "{}, {}",
        super::member_name(member),
        super::member_state(member)
    ))]);
    line
}
