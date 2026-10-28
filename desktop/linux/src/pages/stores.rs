//! Plugin stores: the stores this device joined, as a boxed list, each opening its own page
//! with who runs it, its key and its plugins, and its menu: an owner's tools (see
//! [`store_owner`]), Link another device and Leave. Join in the header bar; the legal
//! line at the foot of the list.

use std::{cell::Cell, rc::Rc};

use adw::prelude::*;
use anistream_i18n::t;
use anistream_model::{
    command::Command,
    event::Event,
    stores::{StorePluginRow, StorePluginStatus, StoreRow, StoreState},
};
use gtk::{gio, glib};

use crate::{
    nagare,
    pages::{plugin, store_owner},
    widgets,
    window::{Ctx, Size},
};

/// Who runs a store and where it answers: "Run by Ren · plugins.example", or its host alone.
pub fn run_by(owners: &[String], host: &str) -> String {
    if owners.is_empty() { host.into() } else { t::stores_run_by(&owners.join(", "), host) }
}

/// The word beside a store, and its mark: access removed, offline, or updates waiting.
pub fn standing_words(store: &StoreRow) -> Option<(String, &'static str)> {
    match &store.state {
        StoreState::Revoked => Some((t::stores_status_access_ended(), "vermilion")),
        StoreState::Offline(_) => Some((t::stores_status_offline(), "ring")),
        StoreState::Ready => match store.updates() {
            0 => None,
            n => Some((t::stores_state_updates(n as i64), "water")),
        },
    }
}

/// What a store's plugin row says under its name.
fn plugin_line(plugin: &StorePluginRow) -> String {
    let contacts = plugin::contacts_line(&plugin.hosts);
    match plugin.status {
        StorePluginStatus::AddedByHand => t::stores_added_by_hand(&contacts),
        _ => contacts,
    }
}

struct Page {
    ctx: Rc<Ctx>,
    body: gtk::Stack,
    list: gtk::ListBox,
    join: gtk::Button,
    shown: Cell<bool>,
}

pub fn build(ctx: &Rc<Ctx>) -> adw::NavigationPage {
    let join = gtk::Button::with_label(&t::stores_join_menu());
    join.set_action_name(Some("win.join-store"));
    let header = adw::HeaderBar::new();
    header.pack_start(&widgets::sidebar_button(&ctx.split));
    header.pack_start(&crate::window::tv_session_button(ctx));
    header.pack_end(&join);

    let list = widgets::boxed_list();
    let column = gtk::Box::new(gtk::Orientation::Vertical, 12);
    column.append(&widgets::heading(&t::stores_joined()));
    column.append(&list);
    column.append(&widgets::haze(&t::stores_not_from_app(), "caption"));
    let legal = widgets::haze(&t::legal_hosts_no_content(), "caption");
    legal.set_margin_top(12);
    column.append(&legal);

    let empty_join = gtk::Button::with_label(&t::stores_join_menu());
    empty_join.add_css_class("pill");
    empty_join.add_css_class("suggested-action");
    empty_join.set_action_name(Some("win.join-store"));
    let empty = nagare::status_page(
        &t::stores_empty_title(),
        &t::stores_empty_detail(),
        Some(&empty_join),
    );
    let body = gtk::Stack::new();
    body.add_named(&widgets::column_page(&column, 720), Some("list"));
    body.add_named(&empty, Some("empty"));
    let navigation = widgets::content_page(&t::nav_plugin_stores(), &header, &body);

    let page = Rc::new(Page { ctx: Rc::clone(ctx), body, list, join, shown: Cell::new(false) });
    let open = gio::SimpleAction::new("open", Some(glib::VariantTy::STRING));
    open.connect_activate({
        let weak = Rc::downgrade(&page);
        move |_, target| {
            if let (Some(page), Some(id)) =
                (weak.upgrade(), target.and_then(String::from_variant))
            {
                push_store(&page.ctx, &id);
            }
        }
    });
    let group = gio::SimpleActionGroup::new();
    group.add_action(&open);
    navigation.insert_action_group("stores", Some(&group));

    let weak = Rc::downgrade(&page);
    ctx.hub.listen(move |event| {
        let Some(page) = weak.upgrade() else { return };
        match event {
            Event::Stores(_) if page.shown.get() => page.show(),
            Event::StoreJoined { store } => {
                // Into the store just joined, as the list's own row would go.
                if let Some(nav) = page.ctx.nav()
                    && nav.visible_page().is_some_and(|p| p.tag().as_deref() == Some("stores"))
                {
                    push_store(&page.ctx, store);
                }
            }
            _ => {}
        }
    });
    ctx.on_size({
        let weak = Rc::downgrade(&page);
        move |size| {
            if let Some(page) = weak.upgrade() {
                if size == Size::Narrow {
                    page.join.set_icon_name("list-add-symbolic");
                    page.join.set_tooltip_text(Some(&t::stores_join_menu()));
                } else {
                    page.join.set_label(&t::stores_join_menu());
                    page.join.set_tooltip_text(None);
                }
            }
        }
    });
    navigation.set_tag(Some("stores"));
    navigation.connect_map({
        let weak = Rc::downgrade(&page);
        move |_| {
            if let Some(page) = weak.upgrade() {
                page.shown.set(true);
                page.ctx.hub.send(Command::LoadStores);
                page.show();
            }
        }
    });
    widgets::keep(&navigation, page);
    navigation
}

impl Page {
    fn show(&self) {
        let stores = self.ctx.hub.known.borrow().stores.clone();
        while let Some(child) = self.list.first_child() {
            self.list.remove(&child);
        }
        self.body.set_visible_child_name(if stores.is_empty() { "empty" } else { "list" });
        for store in &stores {
            let row = adw::ActionRow::builder()
                .title(&store.name)
                .subtitle(run_by(&store.owners, &store.host))
                .activatable(true)
                .action_name("stores.open")
                .action_target(&store.id.to_variant())
                .build();
            row.set_title_lines(1);
            row.set_subtitle_lines(1);
            if let Some((words, mark)) = standing_words(store) {
                row.add_suffix(&widgets::standing(&words, mark));
            }
            row.add_suffix(&widgets::chevron());
            self.list.append(&row);
        }
    }
}

/// One of the store menu's dialogs or actions, for the store it is opened from.
type OwnerTool = fn(&Rc<Ctx>, &str);

/// Push the page of the joined store `id`.
pub fn push_store(ctx: &Rc<Ctx>, id: &str) {
    let Some(nav) = ctx.nav() else { return };
    let menu = gio::Menu::new();
    let row = ctx.hub.known.borrow().stores.iter().find(|s| s.id == id).cloned();
    if let Some(row) = row.filter(|row| row.state != StoreState::Revoked) {
        if row.owner {
            let owner = gio::Menu::new();
            owner.append(Some(&t::stores_invite_someone()), Some("store.invite"));
            owner.append(Some(&t::stores_show_members()), Some("store.members"));
            owner.append(Some(&t::stores_who_can_join_menu()), Some("store.joining"));
            owner.append(Some(&t::stores_publish_menu()), Some("store.publish"));
            owner.append(Some(&t::stores_open_admin()), Some("store.admin"));
            menu.append_section(None, &owner);
        }
        // A device that joined with the store's link joins another the same way.
        if !row.anonymous {
            let device = gio::Menu::new();
            device.append(Some(&t::stores_link_device()), Some("store.device"));
            menu.append_section(None, &device);
        }
    }
    let leave = gio::Menu::new();
    leave.append(Some(&t::stores_leave()), Some("store.leave"));
    menu.append_section(None, &leave);
    let more = gtk::MenuButton::builder()
        .icon_name("view-more-symbolic")
        .tooltip_text(t::action_more())
        .menu_model(&menu)
        .build();
    let header = adw::HeaderBar::new();
    header.pack_end(&more);
    let column = gtk::Box::new(gtk::Orientation::Vertical, 0);
    let scroll = widgets::column_page(&column, 720);
    let page = widgets::page("", &header, &scroll);

    let leave = gio::SimpleAction::new("leave", None);
    leave.connect_activate({
        let ctx = Rc::downgrade(ctx);
        let id = id.to_owned();
        move |_, _| {
            if let Some(ctx) = ctx.upgrade() {
                confirm_leave(&ctx, &id);
            }
        }
    });
    let group = gio::SimpleActionGroup::new();
    group.add_action(&leave);
    let tools: [(&str, OwnerTool); 6] = [
        ("invite", |ctx, id| store_owner::invite(ctx, id, false)),
        ("device", |ctx, id| store_owner::invite(ctx, id, true)),
        ("members", store_owner::members),
        ("joining", store_owner::joining),
        ("publish", store_owner::publish),
        ("admin", |ctx, id| ctx.hub.send(Command::OpenAdmin { store: id.to_owned() })),
    ];
    for (name, open) in tools {
        let action = gio::SimpleAction::new(name, None);
        let ctx = Rc::downgrade(ctx);
        let id = id.to_owned();
        action.connect_activate(move |_, _| {
            if let Some(ctx) = ctx.upgrade() {
                open(&ctx, &id);
            }
        });
        group.add_action(&action);
    }
    page.insert_action_group("store", Some(&group));

    let refresh = {
        let ctx = Rc::downgrade(ctx);
        let page = page.downgrade();
        let column = column.downgrade();
        let id = id.to_owned();
        move || {
            let (Some(ctx), Some(page), Some(column)) =
                (ctx.upgrade(), page.upgrade(), column.upgrade())
            else {
                return;
            };
            while let Some(child) = column.first_child() {
                column.remove(&child);
            }
            let store = ctx.hub.known.borrow().stores.iter().find(|s| s.id == id).cloned();
            match store {
                Some(store) => {
                    page.set_title(&store.name);
                    fill(&ctx, &column, &store);
                }
                None => column.append(&widgets::haze(&t::stores_no_longer_joined(), "")),
            }
        }
    };
    refresh();
    let listening = ctx.hub.listen(move |event| {
        if matches!(event, Event::Stores(_)) {
            refresh();
        }
    });
    let hub = Rc::downgrade(&ctx.hub);
    page.connect_destroy(move |_| {
        if let Some(hub) = hub.upgrade() {
            hub.forget(listening);
        }
    });
    nav.push(&page);
}

fn fill(ctx: &Rc<Ctx>, column: &gtk::Box, store: &StoreRow) {
    let title = gtk::Label::new(Some(&store.name));
    title.add_css_class("title-2");
    title.set_xalign(0.0);
    title.set_wrap(true);
    column.append(&title);
    let by = widgets::haze(&run_by(&store.owners, &store.host), "");
    by.set_margin_top(2);
    column.append(&by);
    if store.open {
        let open = widgets::haze(&t::stores_anyone_with_link(), "caption");
        open.set_margin_top(2);
        column.append(&open);
    }
    let problem = match &store.state {
        StoreState::Offline(reason) => Some((t::stores_offline_last_list(reason), "ring")),
        StoreState::Revoked => Some((t::stores_access_ended_page(), "vermilion")),
        StoreState::Ready => None,
    };
    if let Some((line, mark)) = problem {
        let state = widgets::standing(&line, mark);
        state.set_halign(gtk::Align::Start);
        state.set_margin_top(14);
        if let Some(label) = state.last_child().and_downcast::<gtk::Label>() {
            label.set_wrap(true);
            label.set_xalign(0.0);
            if mark == "vermilion" {
                label.add_css_class("problem");
            }
        }
        column.append(&state);
    }

    let plugins = widgets::heading(&t::stores_plugins());
    plugins.set_margin_top(24);
    plugins.set_margin_bottom(12);
    column.append(&plugins);
    let list = widgets::boxed_list();
    let live = store.state == StoreState::Ready;
    for plugin in &store.plugins {
        list.append(&plugin_row(ctx, store, plugin, live));
    }
    column.append(&list);
    let note = widgets::haze(&t::stores_plugin_limits(), "caption");
    note.set_margin_top(8);
    column.append(&note);
    let key = widgets::haze(
        &t::stores_key_line(
            &store.fingerprint,
            &if store.anonymous {
                t::stores_joined_nameless()
            } else {
                t::stores_device_joined_as(&store.member)
            },
        ),
        "caption",
    );
    key.set_margin_top(4);
    key.set_selectable(true);
    column.append(&key);
}

fn plugin_row(
    ctx: &Rc<Ctx>,
    store: &StoreRow,
    plugin: &StorePluginRow,
    live: bool,
) -> adw::ActionRow {
    let row = adw::ActionRow::builder()
        .title(&plugin.name)
        .subtitle(plugin_line(plugin))
        .activatable(true)
        .build();
    row.set_title_lines(1);
    row.set_subtitle_lines(1);
    let action = |label: &str, tip: String, amber: bool| {
        let button = gtk::Button::with_label(label);
        button.set_valign(gtk::Align::Center);
        if amber {
            button.add_css_class("suggested-action");
        }
        let ctx = Rc::clone(ctx);
        button.set_tooltip_text(Some(&tip));
        let (store, plugin) = (store.clone(), plugin.clone());
        button.connect_clicked(move |_| plugin::install(&ctx, &store, &plugin));
        button
    };
    let word = |text: &str| widgets::haze(text, "caption");
    match &plugin.status {
        StorePluginStatus::Installed => {
            row.add_suffix(&word(&t::stores_plugin_state_installed()))
        }
        StorePluginStatus::OtherStore(other) => {
            row.add_suffix(&word(&t::stores_plugin_from(other)))
        }
        _ if !live => {
            row.add_suffix(&word(&if matches!(store.state, StoreState::Offline(_)) {
                t::stores_plugin_offline()
            } else {
                t::stores_plugin_unavailable()
            }))
        }
        StorePluginStatus::Update { .. } => row.add_suffix(&action(
            &t::action_update(),
            t::stores_update_label(&plugin.name, &plugin.version),
            true,
        )),
        StorePluginStatus::Available => row.add_suffix(&action(
            &t::action_install(),
            t::stores_install_label(&plugin.name),
            false,
        )),
        StorePluginStatus::AddedByHand => row.add_suffix(&action(
            &t::action_replace(),
            t::stores_replace_label(&plugin.name),
            false,
        )),
    }
    row.add_suffix(&widgets::chevron());
    let ctx = Rc::downgrade(ctx);
    let (store, id) = (store.id.clone(), plugin.id.clone());
    row.connect_activated(move |_| {
        if let Some(ctx) = ctx.upgrade() {
            plugin::push_store_plugin(&ctx, &store, &id);
        }
    });
    row
}

fn confirm_leave(ctx: &Rc<Ctx>, id: &str) {
    let name = ctx
        .hub
        .known
        .borrow()
        .stores
        .iter()
        .find(|s| s.id == id)
        .map(|s| s.name.clone())
        .unwrap_or_else(|| id.into());
    let dialog = adw::AlertDialog::new(
        Some(&t::stores_leave_title(&name)),
        Some(&t::stores_leave_detail()),
    );
    dialog.add_response("cancel", &t::action_cancel());
    dialog.add_response("leave", &t::action_leave());
    dialog.set_response_appearance("leave", adw::ResponseAppearance::Destructive);
    dialog.set_default_response(Some("cancel"));
    dialog.set_close_response("cancel");
    let weak = Rc::downgrade(ctx);
    let id = id.to_owned();
    dialog.connect_response(None, move |_, response| {
        let (Some(ctx), "leave") = (weak.upgrade(), response) else { return };
        ctx.hub.send(Command::LeaveStore { store: id.clone() });
        if let Some(nav) = ctx.nav() {
            nav.pop();
        }
    });
    dialog.present(Some(&ctx.window));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_store_says_who_runs_it_and_how_it_stands() {
        assert_eq!(run_by(&[], "plugins.example"), "plugins.example");
        assert_eq!(run_by(&["Ren".into()], "plugins.example"), "Run by Ren · plugins.example");
        let mut store = StoreRow {
            id: "a".into(),
            name: "A".into(),
            url: String::new(),
            host: String::new(),
            fingerprint: String::new(),
            member: String::new(),
            owner: false,
            state: StoreState::Revoked,
            checked_at: None,
            plugins: vec![],
            owners: vec![],
            open: false,
            anonymous: false,
        };
        assert_eq!(standing_words(&store), Some(("Access ended".into(), "vermilion")));
        store.state = StoreState::Ready;
        assert_eq!(standing_words(&store), None);
        let by_hand = StorePluginRow {
            id: "p".into(),
            name: "P".into(),
            version: "1".into(),
            hosts: vec!["films.example".into()],
            status: StorePluginStatus::AddedByHand,
        };
        assert_eq!(plugin_line(&by_hand), "Added by hand · Contacts films.example");
    }
}
