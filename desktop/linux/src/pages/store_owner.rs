//! A store owner's tools, from the store page's menu, as the Mac has them: an invite link for
//! someone new or for another device of yours, the members and the invites not used yet, who can
//! join (only people invited, or anyone with the store's link), and the plugins on this computer
//! to publish there. Each a dialog, which a narrow window shows as a bottom sheet. The engine
//! talks to the store; these only ask it and show what it answers.

use std::{
    cell::{Cell, RefCell},
    rc::Rc,
    time::Duration,
};

use adw::prelude::*;
use anistream_model::{
    command::Command,
    event::Event,
    stores::{InviteLinkView, LocalPluginRow, OpenJoinView, StoreInviteRow, StoreMemberRow},
    wording::{ago, plural},
};
use gtk::glib;

use crate::{widgets, window::Ctx};

/// How long Who Can Join waits, once the store has listed its members, before taking it for a
/// server too old to offer open joining.
const OLDER_SERVER: Duration = Duration::from_secs(4);

/// How long a dialog waits on the store before saying it did not answer.
const NO_ANSWER: Duration = Duration::from_secs(20);

/// A dialog's frame: the header with its buttons, and a column to fill.
struct Frame {
    dialog: adw::Dialog,
    header: adw::HeaderBar,
    body: gtk::Box,
}

/// `title` short, over the store's name, so a narrow window's sheet shows both whole.
fn frame(ctx: &Ctx, name: &str, title: &str, store: &str) -> Frame {
    if let Some(dialog) = ctx.window.visible_dialog()
        && dialog.widget_name() == name
    {
        dialog.force_close();
    }
    let header = adw::HeaderBar::new();
    header.set_show_end_title_buttons(false);
    header.set_show_start_title_buttons(false);
    header.set_title_widget(Some(&adw::WindowTitle::new(title, store)));
    let body = gtk::Box::new(gtk::Orientation::Vertical, 12);
    body.set_margin_top(12);
    body.set_margin_bottom(24);
    body.set_margin_start(18);
    body.set_margin_end(18);
    if ctx.size() != crate::window::Size::Narrow {
        body.set_size_request(460, -1);
    }
    let scroll = gtk::ScrolledWindow::builder()
        .hscrollbar_policy(gtk::PolicyType::Never)
        .propagate_natural_height(true)
        .max_content_height(620)
        .child(&body)
        .build();
    let view = adw::ToolbarView::new();
    view.add_top_bar(&header);
    view.set_content(Some(&scroll));
    let dialog = adw::Dialog::builder()
        .name(name)
        .title(format!("{title}, {store}"))
        .follows_content_size(true)
        .child(&view)
        .build();
    Frame { dialog, header, body }
}

/// A Done button at the header's end, which closes the dialog.
fn done(frame: &Frame) -> gtk::Button {
    let done = gtk::Button::with_label("Done");
    let dialog = frame.dialog.downgrade();
    done.connect_clicked(move |_| {
        if let Some(dialog) = dialog.upgrade() {
            dialog.close();
        }
    });
    frame.header.pack_end(&done);
    frame.dialog.set_default_widget(Some(&done));
    done
}

/// Hear the engine while the dialog is up.
fn listen(ctx: &Ctx, dialog: &adw::Dialog, heard: impl Fn(&Event) + 'static) {
    let listening = ctx.hub.listen(heard);
    let hub = Rc::downgrade(&ctx.hub);
    dialog.connect_closed(move |_| {
        if let Some(hub) = hub.upgrade() {
            hub.forget(listening);
        }
    });
}

/// The engine's alerts, said in the dialog while `claim` takes them, until it closes.
fn claim(ctx: &Rc<Ctx>, dialog: &adw::Dialog, claim: impl Fn(&str) -> bool + 'static) {
    let id = ctx.claim_alerts(claim);
    let ctx = Rc::downgrade(ctx);
    dialog.connect_closed(move |_| {
        if let Some(ctx) = ctx.upgrade() {
            ctx.release_alerts(id);
        }
    });
}

/// A dialog's question to the store: a spinner while it is out, and when it goes unanswered,
/// why, with Try Again. Offline, it is not sent at all.
struct Asking {
    ctx: Rc<Ctx>,
    body: gtk::Box,
    send: Box<dyn Fn(&Ctx)>,
    waiting: Cell<bool>,
    turn: Cell<u64>,
}

impl Asking {
    fn new(
        ctx: &Rc<Ctx>,
        dialog: &adw::Dialog,
        body: &gtk::Box,
        send: impl Fn(&Ctx) + 'static,
    ) -> Rc<Self> {
        let asking = Rc::new(Self {
            ctx: Rc::clone(ctx),
            body: body.clone(),
            send: Box::new(send),
            waiting: Cell::new(false),
            turn: Cell::new(0),
        });
        claim(ctx, dialog, {
            let asking = Rc::downgrade(&asking);
            move |why| {
                let Some(asking) = asking.upgrade() else { return false };
                if !asking.waiting.get() {
                    return false;
                }
                asking.failed("Couldn't Reach the Store", why);
                true
            }
        });
        listen(ctx, dialog, {
            let asking = Rc::downgrade(&asking);
            move |event| {
                if let (Some(asking), Event::Connectivity { offline: true }) =
                    (asking.upgrade(), event)
                    && asking.waiting.get()
                {
                    asking.offline();
                }
            }
        });
        widgets::keep(dialog, Rc::clone(&asking));
        asking
    }

    fn ask(self: &Rc<Self>) {
        empty(&self.body);
        if self.ctx.hub.known.borrow().offline {
            self.offline();
            return;
        }
        self.body.append(&waiting());
        self.waiting.set(true);
        let turn = self.turn.get() + 1;
        self.turn.set(turn);
        (self.send)(&self.ctx);
        let asking = Rc::downgrade(self);
        glib::timeout_add_local_once(NO_ANSWER, move || {
            if let Some(asking) = asking.upgrade()
                && asking.waiting.get()
                && asking.turn.get() == turn
            {
                asking.failed(
                    "The Store Didn't Answer",
                    "It may be busy, or its computer may be off.",
                );
            }
        });
    }

    /// The store answered: what it said replaces the spinner.
    fn answered(&self) {
        self.waiting.set(false);
    }

    fn offline(self: &Rc<Self>) {
        self.failed(
            "The Store Needs a Connection",
            "Try again once this computer is back online.",
        );
    }

    fn failed(self: &Rc<Self>, title: &str, why: &str) {
        self.waiting.set(false);
        self.turn.set(self.turn.get() + 1);
        empty(&self.body);
        let again = gtk::Button::with_label("Try Again");
        again.add_css_class("pill");
        again.set_halign(gtk::Align::Center);
        let asking = Rc::downgrade(self);
        again.connect_clicked(move |_| {
            if let Some(asking) = asking.upgrade() {
                asking.ask();
            }
        });
        let page = crate::states::plain(title, why);
        page.set_child(Some(&again));
        self.body.append(&page);
    }
}

fn empty(column: &gtk::Box) {
    while let Some(child) = column.first_child() {
        column.remove(&child);
    }
}

/// A spinner in the dialog's middle, while the store is asked.
fn waiting() -> gtk::Widget {
    let spinner = adw::Spinner::new();
    spinner.set_size_request(24, 24);
    spinner.set_halign(gtk::Align::Center);
    spinner.set_margin_top(18);
    spinner.set_margin_bottom(18);
    spinner.upcast()
}

/// The engine's words for what went wrong, in the error colour.
fn problem_line(text: &str) -> gtk::Label {
    let label = gtk::Label::new(Some(text));
    label.add_css_class("error");
    label.set_xalign(0.0);
    label.set_wrap(true);
    label
}

/// A text button on a row's end.
fn row_button(label: &str) -> gtk::Button {
    let button = gtk::Button::with_label(label);
    button.set_valign(gtk::Align::Center);
    button
}

/// A link on a plate, in monospace, selectable, with Copy Link under it.
fn link_plate(link: &str) -> gtk::Box {
    let text = gtk::Label::new(Some(link));
    text.add_css_class("monospace");
    text.set_xalign(0.0);
    text.set_wrap(true);
    text.set_wrap_mode(gtk::pango::WrapMode::Char);
    // A link broken across lines gains no hyphen, so what is copied by hand stays the link.
    let attrs = gtk::pango::AttrList::new();
    attrs.insert(gtk::pango::AttrInt::new_insert_hyphens(false));
    text.set_attributes(Some(&attrs));
    text.set_selectable(true);
    text.set_hexpand(true);
    let plate = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    plate.add_css_class("link-plate");
    plate.append(&text);
    let copy = gtk::Button::with_label("Copy Link");
    copy.set_halign(gtk::Align::End);
    let link = link.to_owned();
    copy.connect_clicked(move |button| {
        button.clipboard().set_text(&link);
        button.set_label("Copied");
        let button = button.downgrade();
        glib::timeout_add_local_once(Duration::from_secs(2), move || {
            if let Some(button) = button.upgrade() {
                button.set_label("Copy Link");
            }
        });
    });
    let column = gtk::Box::new(gtk::Orientation::Vertical, 8);
    column.append(&plate);
    column.append(&copy);
    column
}

/// When an invite stops working, as a date and a time: "28 Oct, 20:15".
pub fn expiry(seconds: i64) -> String {
    let twelve = crate::pages::calendar::twelve_hour();
    glib::DateTime::from_unix_local(seconds)
        .ok()
        .and_then(|when| {
            when.format(if twelve { "%-d %b, %-l:%M %p" } else { "%-d %b, %H:%M" }).ok()
        })
        .map(|text| text.to_string())
        .unwrap_or_default()
}

/// How long ago, from the store's seconds.
fn since(seconds: i64) -> String {
    ago(glib::DateTime::now_utc().map(|now| now.to_unix()).unwrap_or(seconds) - seconds)
}

fn store_name(ctx: &Ctx, store: &str) -> String {
    ctx.hub
        .known
        .borrow()
        .stores
        .iter()
        .find(|s| s.id == store)
        .map(|s| s.name.clone())
        .unwrap_or_else(|| store.to_owned())
}

// Invite.

/// An invite link to make: for someone new, by name, or for another device of yours.
pub fn invite(ctx: &Rc<Ctx>, store: &str, device: bool) {
    let name = store_name(ctx, store);
    let title = if device { "Link Another Device" } else { "Invite Someone" };
    let frame = frame(ctx, "store-invite", title, &name);
    let cancel = gtk::Button::with_label("Cancel");
    frame.header.pack_start(&cancel);
    let make = gtk::Button::with_label("Make Link");
    make.add_css_class("suggested-action");
    frame.header.pack_end(&make);
    let spinner = adw::Spinner::new();
    spinner.set_visible(false);
    frame.header.pack_end(&spinner);

    let field = adw::EntryRow::builder().title("Their name").build();
    // Read when Make Link is pressed, so kept with the dialog even where it is not shown.
    widgets::keep(&frame.dialog, field.clone());
    let problem = problem_line("");
    problem.set_visible(false);
    if device {
        frame.body.append(&widgets::haze("Make a link to open on your other device.", ""));
    } else {
        let list = widgets::boxed_list();
        list.append(&field);
        frame.body.append(&list);
        make.set_sensitive(false);
        field.connect_changed({
            let make = make.downgrade();
            move |field| {
                if let Some(make) = make.upgrade() {
                    make.set_sensitive(!field.text().trim().is_empty());
                }
            }
        });
    }
    frame.body.append(&problem);
    frame.dialog.set_default_widget(Some(&make));
    field.connect_entry_activated({
        let make = make.downgrade();
        move |_| {
            if let Some(make) = make.upgrade()
                && make.is_sensitive()
            {
                make.emit_clicked();
            }
        }
    });
    cancel.connect_clicked({
        let dialog = frame.dialog.downgrade();
        move |_| {
            if let Some(dialog) = dialog.upgrade() {
                dialog.close();
            }
        }
    });
    let asked = Rc::new(Cell::new(false));
    make.connect_clicked({
        let ctx = Rc::downgrade(ctx);
        let (store, field, spinner, asked, problem) = (
            store.to_owned(),
            field.downgrade(),
            spinner.downgrade(),
            Rc::clone(&asked),
            problem.downgrade(),
        );
        move |make| {
            let (Some(ctx), Some(field), Some(spinner), Some(problem)) =
                (ctx.upgrade(), field.upgrade(), spinner.upgrade(), problem.upgrade())
            else {
                return;
            };
            let name = (!device).then(|| field.text().trim().to_owned());
            if ctx.hub.known.borrow().offline {
                problem.set_label("Making a link needs a connection to the store.");
                problem.set_visible(true);
                return;
            }
            problem.set_visible(false);
            asked.set(true);
            make.set_visible(false);
            spinner.set_visible(true);
            field.set_sensitive(false);
            ctx.hub.send(Command::CreateInvite { store: store.clone(), name });
        }
    });
    listen(ctx, &frame.dialog, {
        let (store, body, header) =
            (store.to_owned(), frame.body.downgrade(), frame.header.downgrade());
        let (make, cancel, spinner) =
            (make.downgrade(), cancel.downgrade(), spinner.downgrade());
        let dialog = frame.dialog.downgrade();
        let asked = Rc::clone(&asked);
        move |event| {
            let Event::InviteLink(link) = event else { return };
            if link.store != store || !asked.get() {
                return;
            }
            let (
                Some(body),
                Some(header),
                Some(make),
                Some(cancel),
                Some(spinner),
                Some(dialog),
            ) = (
                body.upgrade(),
                header.upgrade(),
                make.upgrade(),
                cancel.upgrade(),
                spinner.upgrade(),
                dialog.upgrade(),
            )
            else {
                return;
            };
            asked.set(false);
            header.remove(&make);
            header.remove(&cancel);
            header.remove(&spinner);
            let frame = Frame { dialog, header, body: body.clone() };
            done(&frame);
            show_link(&body, link, device);
        }
    });
    // Why the engine could not, said here and not in a toast under the dialog: the dialog is
    // as it was, the name kept, to try again.
    claim(ctx, &frame.dialog, {
        let (make, spinner, field, problem) =
            (make.downgrade(), spinner.downgrade(), field.downgrade(), problem.downgrade());
        move |why| {
            if !asked.get() {
                return false;
            }
            asked.set(false);
            if let (Some(make), Some(spinner), Some(field), Some(problem)) =
                (make.upgrade(), spinner.upgrade(), field.upgrade(), problem.upgrade())
            {
                spinner.set_visible(false);
                make.set_visible(true);
                field.set_sensitive(true);
                problem.set_label(why);
                problem.set_visible(true);
            }
            true
        }
    });
    frame.dialog.present(Some(&ctx.window));
    if !device {
        field.grab_focus();
    }
}

fn show_link(body: &gtk::Box, link: &InviteLinkView, device: bool) {
    empty(body);
    let until = expiry(link.expires_at);
    let line = if device {
        format!("Open this link on the other device. It works once, until {until}.")
    } else {
        format!(
            "Send this link to {}. It works once, until {until}.",
            link.for_whom.as_deref().unwrap_or("them")
        )
    };
    body.append(&widgets::haze(&line, ""));
    body.append(&link_plate(&link.link.0));
}

// Members.

/// A store's members and the invites not used yet.
pub fn members(ctx: &Rc<Ctx>, store: &str) {
    let name = store_name(ctx, store);
    let frame = frame(ctx, "store-members", "Members", &name);
    done(&frame);
    let asking = Asking::new(ctx, &frame.dialog, &frame.body, {
        let store = store.to_owned();
        move |ctx| ctx.hub.send(Command::LoadMembers { store: store.clone() })
    });
    listen(ctx, &frame.dialog, {
        let ctx = Rc::downgrade(ctx);
        let (store, body) = (store.to_owned(), frame.body.downgrade());
        let asking = Rc::downgrade(&asking);
        move |event| {
            let (Some(ctx), Some(body)) = (ctx.upgrade(), body.upgrade()) else { return };
            if let Event::StoreMembers { store: of, members, invites } = event
                && *of == store
            {
                if let Some(asking) = asking.upgrade() {
                    asking.answered();
                }
                show_members(&ctx, &body, &store, members, invites);
            }
        }
    });
    asking.ask();
    frame.dialog.present(Some(&ctx.window));
}

fn show_members(
    ctx: &Rc<Ctx>,
    body: &gtk::Box,
    store: &str,
    members: &[StoreMemberRow],
    invites: &[StoreInviteRow],
) {
    empty(body);
    let list = widgets::boxed_list();
    for member in members {
        let devices = plural(member.devices as usize, "device");
        let row = adw::ActionRow::builder()
            .title(if member.you {
                format!("{} (you)", member.name)
            } else {
                member.name.clone()
            })
            .subtitle(if member.owner { format!("Owner · {devices}") } else { devices })
            .build();
        if !member.you {
            let remove = row_button("Remove…");
            remove.set_tooltip_text(Some(&format!("Remove {}", member.name)));
            let ctx = Rc::downgrade(ctx);
            let (store, member) = (store.to_owned(), member.clone());
            remove.connect_clicked(move |button| {
                if let Some(ctx) = ctx.upgrade() {
                    confirm_remove(&ctx, &store, &member, button);
                }
            });
            row.add_suffix(&remove);
        }
        list.append(&row);
    }
    body.append(&list);
    if invites.is_empty() {
        body.append(&widgets::haze(
            "No invites waiting. An invite link shows here until it is used.",
            "caption",
        ));
        return;
    }
    let heading = widgets::heading("Invites Not Used Yet");
    heading.set_margin_top(12);
    body.append(&heading);
    let list = widgets::boxed_list();
    for invite in invites {
        let row = adw::ActionRow::builder()
            .title(if invite.device { "Another device" } else { &invite.name })
            .subtitle(format!("Invited, until {}", expiry(invite.expires_at)))
            .build();
        let cancel = row_button("Cancel Invite");
        let ctx = Rc::downgrade(ctx);
        let (store, id) = (store.to_owned(), invite.id.clone());
        cancel.connect_clicked(move |button| {
            if let Some(ctx) = ctx.upgrade() {
                button.set_sensitive(false);
                ctx.hub
                    .send(Command::CancelInvite { store: store.clone(), invite: id.clone() });
            }
        });
        row.add_suffix(&cancel);
        list.append(&row);
    }
    body.append(&list);
}

fn confirm_remove(ctx: &Rc<Ctx>, store: &str, member: &StoreMemberRow, button: &gtk::Button) {
    let dialog = adw::AlertDialog::new(
        Some(&format!("Remove {} from {}?", member.name, store_name(ctx, store))),
        Some(&format!(
            "Their {} lose access to this store.",
            plural(member.devices as usize, "device")
        )),
    );
    dialog.add_response("cancel", "Cancel");
    dialog.add_response("remove", "Remove Member");
    dialog.set_response_appearance("remove", adw::ResponseAppearance::Destructive);
    dialog.set_default_response(Some("cancel"));
    dialog.set_close_response("cancel");
    let weak = Rc::downgrade(ctx);
    let (store, id, button) = (store.to_owned(), member.id.clone(), button.downgrade());
    dialog.connect_response(None, move |_, response| {
        let (Some(ctx), "remove") = (weak.upgrade(), response) else { return };
        if let Some(button) = button.upgrade() {
            button.set_sensitive(false);
        }
        ctx.hub.send(Command::RemoveMember { store: store.clone(), member: id.clone() });
    });
    dialog.present(Some(&ctx.window));
}

// Who can join.

struct Joining {
    ctx: Rc<Ctx>,
    store: String,
    body: gtk::Box,
    /// Set while the switch is moved to show the store's answer, not by the viewer.
    showing: Cell<bool>,
    /// Whether the store has answered, so the older server's words never replace an answer.
    answered: Cell<bool>,
    asking: RefCell<Option<Rc<Asking>>>,
}

/// Who can join a store: only people invited, or anyone with the store's link, and the devices
/// that joined with it.
pub fn joining(ctx: &Rc<Ctx>, store: &str) {
    let name = store_name(ctx, store);
    let frame = frame(ctx, "store-joining", "Who Can Join", &name);
    done(&frame);
    let state = Rc::new(Joining {
        ctx: Rc::clone(ctx),
        store: store.to_owned(),
        body: frame.body.clone(),
        showing: Cell::new(false),
        answered: Cell::new(false),
        asking: RefCell::default(),
    });
    // The store lists its members first, then says who can join: a store that answers the
    // first and stays silent on the second runs a server from before open joining.
    let asking = Asking::new(ctx, &frame.dialog, &frame.body, {
        let state = Rc::downgrade(&state);
        move |ctx| {
            if let Some(state) = state.upgrade() {
                state.answered.set(false);
                ctx.hub.send(Command::LoadMembers { store: state.store.clone() });
            }
        }
    });
    state.asking.replace(Some(Rc::clone(&asking)));
    listen(ctx, &frame.dialog, {
        let state = Rc::downgrade(&state);
        move |event| {
            let Some(state) = state.upgrade() else { return };
            match event {
                Event::OpenJoin { store, view } if *store == state.store => {
                    if let Some(asking) = state.asking.borrow().as_ref() {
                        asking.answered();
                    }
                    state.show(view);
                }
                Event::StoreMembers { store, .. } if *store == state.store => {
                    state.wait_for_open_join();
                }
                _ => {}
            }
        }
    });
    widgets::keep(&frame.dialog, Rc::clone(&state));
    asking.ask();
    frame.dialog.present(Some(&ctx.window));
}

impl Joining {
    /// The members came; who can join follows at once from a server that knows open joining.
    fn wait_for_open_join(self: &Rc<Self>) {
        let state = Rc::downgrade(self);
        glib::timeout_add_local_once(OLDER_SERVER, move || {
            let Some(state) = state.upgrade() else { return };
            let waiting = state.asking.borrow().as_ref().is_some_and(|a| a.waiting.get());
            if state.answered.get() || !waiting {
                return;
            }
            if let Some(asking) = state.asking.borrow().as_ref() {
                asking.answered();
            }
            empty(&state.body);
            state.body.append(&widgets::haze(
                "This store's server is older, and lets in only people you invite. Updating it \
                 adds the choice.",
                "",
            ));
        });
    }

    fn show(self: &Rc<Self>, view: &OpenJoinView) {
        self.answered.set(true);
        empty(&self.body);
        let list = widgets::boxed_list();
        let open = adw::SwitchRow::builder()
            .title("Anyone with the store's link")
            .subtitle(if view.open {
                "They join without a name. Turning this off stops the link; turning it on again \
                 makes a new one."
            } else {
                "Only people you invite can join. Turned on, anyone with the link joins, without \
                 a name."
            })
            .build();
        self.showing.set(true);
        open.set_active(view.open);
        self.showing.set(false);
        open.connect_active_notify({
            let state = Rc::downgrade(self);
            move |open| {
                let Some(state) = state.upgrade() else { return };
                if state.showing.get() {
                    return;
                }
                open.set_sensitive(false);
                state.ctx.hub.send(Command::SetOpenJoin {
                    store: state.store.clone(),
                    open: open.is_active(),
                });
            }
        });
        list.append(&open);
        self.body.append(&list);
        if let Some(link) = &view.link {
            self.body.append(&link_plate(&link.0));
        }
        if view.devices == 0 {
            return;
        }
        let devices = widgets::boxed_list();
        let summary = adw::ActionRow::builder()
            .title(format!("{} joined with the link", plural(view.devices as usize, "device")))
            .subtitle(if view.open {
                "No names are kept."
            } else {
                "They keep working until you remove them."
            })
            .build();
        let all = row_button(if view.devices == 1 { "Remove" } else { "Remove All" });
        all.connect_clicked({
            let state = Rc::downgrade(self);
            move |_| {
                if let Some(state) = state.upgrade() {
                    state.confirm_remove_all();
                }
            }
        });
        summary.add_suffix(&all);
        devices.append(&summary);
        for device in &view.recent {
            let row = adw::ActionRow::builder()
                .title(format!("Joined {}", since(device.joined_at)))
                .subtitle(device.last_seen.map_or_else(
                    || "Not seen since".to_owned(),
                    |at| format!("Seen {}", since(at)),
                ))
                .build();
            let remove = row_button("Remove");
            let state = Rc::downgrade(self);
            let id = device.id.clone();
            remove.connect_clicked(move |button| {
                if let Some(state) = state.upgrade() {
                    button.set_sensitive(false);
                    state.ctx.hub.send(Command::RemoveAnonymous {
                        store: state.store.clone(),
                        device: Some(id.clone()),
                    });
                }
            });
            row.add_suffix(&remove);
            devices.append(&row);
        }
        devices.set_margin_top(12);
        self.body.append(&devices);
        if view.devices as usize > view.recent.len() {
            self.body.append(&widgets::haze(
                &format!("The newest {} are listed.", view.recent.len()),
                "caption",
            ));
        }
    }

    fn confirm_remove_all(self: &Rc<Self>) {
        let dialog = adw::AlertDialog::new(
            Some("Remove the devices that joined with the link?"),
            Some(&format!(
                "They stop working with {} at once. Plugins already installed on them stay.",
                store_name(&self.ctx, &self.store)
            )),
        );
        dialog.add_response("cancel", "Cancel");
        dialog.add_response("remove", "Remove");
        dialog.set_response_appearance("remove", adw::ResponseAppearance::Destructive);
        dialog.set_default_response(Some("cancel"));
        dialog.set_close_response("cancel");
        let state = Rc::downgrade(self);
        dialog.connect_response(None, move |_, response| {
            let (Some(state), "remove") = (state.upgrade(), response) else { return };
            state
                .ctx
                .hub
                .send(Command::RemoveAnonymous { store: state.store.clone(), device: None });
        });
        dialog.present(Some(&self.ctx.window));
    }
}

// Publish.

/// The plugins on this computer, to publish to the store or bring up to date there.
pub fn publish(ctx: &Rc<Ctx>, store: &str) {
    let name = store_name(ctx, store);
    let frame = frame(ctx, "store-publish", "Publish a Plugin", &name);
    done(&frame);
    let publishing = Rc::new(Publishing::default());
    let asking = Asking::new(ctx, &frame.dialog, &frame.body, {
        let store = store.to_owned();
        move |ctx| ctx.hub.send(Command::LoadLocalPlugins { store: store.clone() })
    });
    listen(ctx, &frame.dialog, {
        let ctx = Rc::downgrade(ctx);
        let (store, body) = (store.to_owned(), frame.body.downgrade());
        let publishing = Rc::clone(&publishing);
        let asking = Rc::downgrade(&asking);
        move |event| {
            let (Some(ctx), Some(body)) = (ctx.upgrade(), body.upgrade()) else { return };
            match event {
                Event::LocalPlugins { store: of, plugins } if *of == store => {
                    if let Some(asking) = asking.upgrade() {
                        asking.answered();
                    }
                    publishing.plugin.replace(None);
                    show_local(&ctx, &body, &store, plugins, &publishing);
                }
                // Published: the store's list changed, so what it offers is asked again.
                Event::Stores(_) if publishing.plugin.borrow().is_some() => {
                    ctx.hub.send(Command::LoadLocalPlugins { store: store.clone() });
                }
                _ => {}
            }
        }
    });
    // Not published: why shows over the list, and the list is asked again.
    claim(ctx, &frame.dialog, {
        let ctx = Rc::downgrade(ctx);
        let store = store.to_owned();
        let publishing = Rc::clone(&publishing);
        move |why| {
            let Some(ctx) = ctx.upgrade() else { return false };
            if publishing.plugin.borrow().is_none() {
                return false;
            }
            // The engine names the plugin by its id: here it goes by the name its row shows.
            let id = publishing.plugin.borrow().clone().unwrap_or_default();
            let named = why
                .strip_prefix(&crate::catalog::shown(&id))
                .map(|rest| format!("{}{rest}", publishing.name.borrow()));
            publishing.problem.replace(Some(named.unwrap_or_else(|| why.to_owned())));
            ctx.hub.send(Command::LoadLocalPlugins { store: store.clone() });
            true
        }
    });
    asking.ask();
    frame.dialog.present(Some(&ctx.window));
}

/// The plugin being published, by id and name, and why the last one was not.
#[derive(Default)]
struct Publishing {
    plugin: RefCell<Option<String>>,
    name: RefCell<String>,
    problem: RefCell<Option<String>>,
}

fn show_local(
    ctx: &Rc<Ctx>,
    body: &gtk::Box,
    store: &str,
    plugins: &[LocalPluginRow],
    publishing: &Rc<Publishing>,
) {
    empty(body);
    if let Some(why) = publishing.problem.take() {
        body.append(&problem_line(&why));
    }
    if plugins.is_empty() {
        body.append(&widgets::haze(
            "No plugins on this computer to publish. A plugin you build or install shows here.",
            "",
        ));
        return;
    }
    let list = widgets::boxed_list();
    for plugin in plugins {
        let row = adw::ActionRow::builder()
            .title(format!("{} {}", plugin.name, plugin.version))
            .subtitle(plugin.published.as_ref().map_or_else(
                || "Not in the store yet".to_owned(),
                |v| format!("The store offers {v}"),
            ))
            .build();
        // This version already in the store: nothing to do, and said so.
        if plugin.published.as_deref() == Some(plugin.version.as_str()) {
            row.set_subtitle("Up to date in the store");
            row.add_suffix(&widgets::haze("Published", ""));
            list.append(&row);
            continue;
        }
        let button = row_button("Publish");
        button.set_tooltip_text(Some(&format!("Publish {} {}", plugin.name, plugin.version)));
        let ctx = Rc::downgrade(ctx);
        let (store, id, name, publishing) =
            (store.to_owned(), plugin.id.clone(), plugin.name.clone(), Rc::clone(publishing));
        button.connect_clicked(move |button| {
            let Some(ctx) = ctx.upgrade() else { return };
            button.set_sensitive(false);
            let spinner = adw::Spinner::new();
            button.set_child(Some(&spinner));
            publishing.plugin.replace(Some(id.clone()));
            publishing.name.replace(name.clone());
            ctx.hub.send(Command::PublishPlugin { store: store.clone(), plugin: id.clone() });
        });
        row.add_suffix(&button);
        list.append(&row);
    }
    body.append(&list);
}
