//! Join a plugin store: the invite link, checked against the store it names as it is pasted, then
//! the store's preview (its name, who runs it, the key, its plugins and what this computer is
//! called there) and Join Store. A dialog, which a narrow window shows as a bottom sheet.

use std::{cell::Cell, rc::Rc, time::Duration};

use adw::prelude::*;
use anistream_model::{
    command::Command,
    event::Event,
    stores::{InvitePreview, InvitedStore, Secret},
};
use gtk::glib;

use crate::{pages::plugin, widgets, window::Ctx};

struct Join {
    ctx: Rc<Ctx>,
    dialog: adw::Dialog,
    link: adw::EntryRow,
    preview: gtk::Box,
    join: gtk::Button,
    /// The store the link leads to, once checked.
    store: std::cell::RefCell<Option<InvitedStore>>,
    device: std::cell::RefCell<Option<adw::EntryRow>>,
    /// Bumped by each edit, so only the last one is checked.
    edits: Cell<u64>,
}

/// Open the dialog, with `link` in it when one was pasted or opened.
pub fn open(ctx: &Rc<Ctx>, link: Option<&str>) {
    let header = adw::HeaderBar::new();
    header.set_show_end_title_buttons(false);
    header.set_show_start_title_buttons(false);
    let cancel = gtk::Button::with_label("Cancel");
    header.pack_start(&cancel);
    let join = gtk::Button::with_label("Join Store");
    join.add_css_class("suggested-action");
    join.set_sensitive(false);
    header.pack_end(&join);

    let column = gtk::Box::new(gtk::Orientation::Vertical, 12);
    column.set_margin_top(12);
    column.set_margin_bottom(24);
    column.set_margin_start(18);
    column.set_margin_end(18);
    column.append(&widgets::haze("Paste the invite link the store's owner sent you.", ""));
    let links = widgets::boxed_list();
    let entry = adw::EntryRow::builder().title("Invite link").build();
    links.append(&entry);
    column.append(&links);
    let preview = gtk::Box::new(gtk::Orientation::Vertical, 12);
    column.append(&preview);
    let note = widgets::haze(
        "Plugins come from the store's owner, not from anistream. You can leave a store at any \
         time.",
        "caption",
    );
    column.append(&note);
    // The dialog follows its content, which grows once the link is checked.
    if ctx.size() != crate::window::Size::Narrow {
        column.set_size_request(440, -1);
    }
    let scroll = gtk::ScrolledWindow::builder()
        .hscrollbar_policy(gtk::PolicyType::Never)
        .propagate_natural_height(true)
        .max_content_height(620)
        .child(&column)
        .build();
    let view = adw::ToolbarView::new();
    view.add_top_bar(&header);
    view.set_content(Some(&scroll));
    let dialog = adw::Dialog::builder()
        .title("Join a Plugin Store")
        .follows_content_size(true)
        .child(&view)
        .build();

    let state = Rc::new(Join {
        ctx: Rc::clone(ctx),
        dialog: dialog.clone(),
        link: entry.clone(),
        preview,
        join: join.clone(),
        store: Default::default(),
        device: Default::default(),
        edits: Cell::new(0),
    });
    cancel.connect_clicked({
        let dialog = dialog.clone();
        move |_| {
            dialog.close();
        }
    });
    join.connect_clicked({
        let weak = Rc::downgrade(&state);
        move |_| {
            if let Some(state) = weak.upgrade() {
                state.join();
            }
        }
    });
    entry.connect_changed({
        let weak = Rc::downgrade(&state);
        move |_| {
            if let Some(state) = weak.upgrade() {
                state.edited();
            }
        }
    });
    let listening = ctx.hub.listen({
        let weak = Rc::downgrade(&state);
        move |event| {
            let Some(state) = weak.upgrade() else { return };
            match event {
                Event::StorePreview(preview) => state.previewed(preview),
                Event::StoreJoined { .. } => {
                    let name = state.store.borrow().as_ref().map(|s| s.name.clone());
                    state.dialog.close();
                    if let Some(name) = name {
                        state.ctx.toast(&format!("Joined {name}"));
                    }
                }
                _ => {}
            }
        }
    });
    let hub = Rc::downgrade(&ctx.hub);
    dialog.connect_closed(move |_| {
        if let Some(hub) = hub.upgrade() {
            hub.forget(listening);
        }
    });
    widgets::keep(&dialog, Rc::clone(&state));
    if let Some(link) = link {
        entry.set_text(link);
    }
    dialog.present(Some(&ctx.window));
    entry.grab_focus();
    entry.set_position(-1);
}

impl Join {
    fn text(&self) -> String {
        self.link.text().trim().to_owned()
    }

    /// Check the link once typing stops.
    fn edited(self: &Rc<Self>) {
        let edit = self.edits.get() + 1;
        self.edits.set(edit);
        self.store.replace(None);
        self.join.set_sensitive(false);
        let link = self.text();
        if link.is_empty() {
            self.clear();
            return;
        }
        self.say("Checking the link…", false);
        let weak = Rc::downgrade(self);
        glib::timeout_add_local_once(Duration::from_millis(400), move || {
            if let Some(state) = weak.upgrade()
                && state.edits.get() == edit
            {
                state.ctx.hub.send(Command::PreviewStore { link: Secret(link) });
            }
        });
    }

    fn clear(&self) {
        while let Some(child) = self.preview.first_child() {
            self.preview.remove(&child);
        }
        self.device.replace(None);
    }

    fn say(&self, line: &str, problem: bool) {
        self.clear();
        let label = widgets::haze(line, "");
        if problem {
            label.add_css_class("problem");
        }
        self.preview.append(&label);
    }

    fn previewed(&self, preview: &InvitePreview) {
        match preview {
            InvitePreview::Failed { reason } => {
                self.store.replace(None);
                self.join.set_sensitive(false);
                self.say(reason, true);
            }
            InvitePreview::Store(store) => {
                if store.link.0.trim() != self.text() {
                    return;
                }
                self.show(store);
                self.join.set_sensitive(!store.already_joined);
                self.store.replace(Some(store.clone()));
            }
        }
    }

    fn show(&self, store: &InvitedStore) {
        self.clear();
        let card = widgets::boxed_list();
        let name = adw::ActionRow::builder()
            .title(&store.name)
            .subtitle(crate::pages::stores::run_by(&store.owners, &store.host))
            .build();
        card.append(&name);
        let key = adw::ActionRow::builder()
            .title("Store key")
            .subtitle(format!(
                "{} · matches the {}",
                store.fingerprint,
                if store.anonymous { "link" } else { "invite" }
            ))
            .build();
        key.add_css_class("property");
        card.append(&key);
        if store.open {
            card.append(
                &adw::ActionRow::builder().title("Anyone with the link can join").build(),
            );
        }
        if !store.anonymous {
            let device = adw::EntryRow::builder().title("Device name").build();
            device.set_text(&store.device_label);
            card.append(&device);
            self.device.replace(Some(device));
        }
        self.preview.append(&card);
        let line = if store.already_joined {
            "This computer has joined this store already.".to_owned()
        } else if store.anonymous {
            "You join without a name. Another computer joins with the same link.".to_owned()
        } else {
            "The store's owner sees this computer by this name.".to_owned()
        };
        self.preview.append(&widgets::haze(&line, "caption"));

        if !store.plugins.is_empty() {
            let count = store.plugins.len();
            let heading = widgets::heading(&if count == 1 {
                "1 Plugin".to_owned()
            } else {
                format!("{count} Plugins")
            });
            heading.set_margin_top(6);
            self.preview.append(&heading);
            let list = widgets::boxed_list();
            for plugin in &store.plugins {
                let row = adw::ActionRow::builder()
                    .title(&plugin.name)
                    .subtitle(plugin::contacts_line(&plugin.hosts))
                    .build();
                row.set_title_lines(1);
                row.set_subtitle_lines(1);
                list.append(&row);
            }
            self.preview.append(&list);
        }
    }

    fn join(&self) {
        let Some(store) = self.store.borrow().clone() else { return };
        let label = self.device.borrow().as_ref().map(|d| d.text().to_string());
        self.join.set_sensitive(false);
        self.ctx.hub.send(Command::JoinStore { link: store.link, label });
    }
}
