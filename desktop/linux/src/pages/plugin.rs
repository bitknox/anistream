//! A source's page, pushed from Providers or from a store: what it is and where it came from,
//! what can be done with it, and its version, response time and the hosts it may contact. Also
//! the confirmations its actions ask: an update names the hosts it adds, a removal says what
//! stays.

use std::rc::Rc;

use adw::prelude::*;
use anistream_core::settings::SettingValue;
use anistream_model::{
    command::Command,
    event::Event,
    settings::SettingKey,
    sources::{ProviderOrigin, ProviderRow},
    stores::{StorePluginRow, StorePluginStatus, StoreRow, StoreState},
};

use crate::{
    hub::Known,
    pages::providers::{self, Standing},
    widgets,
    window::Ctx,
};

/// The tag a plugin's page goes by, so a removal can leave it.
const TAG: &str = "plugin";

/// What a page is about: an installed source by id, or a store's plugin.
#[derive(Clone)]
struct Subject {
    plugin: String,
    store: Option<String>,
}

/// What the page shows, read from what the engine said last.
struct Info {
    row: Option<ProviderRow>,
    listing: Option<(StoreRow, StorePluginRow)>,
}

impl Info {
    fn of(known: &Known, subject: &Subject) -> Option<Self> {
        let row = known.providers.iter().find(|r| r.id == subject.plugin).cloned();
        let listing = match &subject.store {
            Some(store) => known.stores.iter().find(|s| &s.id == store).and_then(|s| {
                let plugin = s.plugins.iter().find(|p| p.id == subject.plugin)?;
                Some((s.clone(), plugin.clone()))
            }),
            None => known.origin(&subject.plugin),
        };
        (row.is_some() || listing.is_some()).then_some(Self { row, listing })
    }

    fn name(&self) -> String {
        match (&self.row, &self.listing) {
            (Some(row), _) => providers::name(row),
            (None, Some((_, plugin))) => plugin.name.clone(),
            (None, None) => String::new(),
        }
    }

    fn origin(&self) -> String {
        if let Some(row) = &self.row {
            return match &row.origin {
                ProviderOrigin::Store { name: Some(name), .. } => format!("From {name}"),
                _ => providers::origin(row),
            };
        }
        match &self.listing {
            Some((store, _)) => format!("In {}", store.name),
            None => String::new(),
        }
    }

    fn hosts(&self) -> Vec<String> {
        match (&self.row, &self.listing) {
            (Some(row), _) if !row.hosts.is_empty() => row.hosts.clone(),
            (_, Some((_, plugin))) => plugin.hosts.clone(),
            _ => Vec::new(),
        }
    }
}

/// "This update also contacts media2.example and 1 more."
pub fn new_hosts_line(hosts: &[String]) -> Option<String> {
    let first = hosts.first()?;
    Some(match hosts.len() {
        1 => format!("This update also contacts {first}."),
        n => format!("This update also contacts {first} and {} more.", n - 1),
    })
}

/// What a store's plugin may contact, in a line: "Contacts films.example and 1 more".
pub fn contacts_line(hosts: &[String]) -> String {
    match hosts {
        [] => "Contacts no hosts".into(),
        [one] => format!("Contacts {one}"),
        [first, rest @ ..] => format!("Contacts {first} and {} more", rest.len()),
    }
}

/// Push the page of the installed source `id`.
pub fn push_provider(ctx: &Rc<Ctx>, id: &str) {
    push(ctx, Subject { plugin: id.into(), store: None });
}

/// Push the page of `plugin` as `store` offers it.
pub fn push_store_plugin(ctx: &Rc<Ctx>, store: &str, plugin: &str) {
    push(ctx, Subject { plugin: plugin.into(), store: Some(store.into()) });
}

fn push(ctx: &Rc<Ctx>, subject: Subject) {
    let Some(nav) = ctx.nav() else { return };
    let header = adw::HeaderBar::new();
    let column = gtk::Box::new(gtk::Orientation::Vertical, 0);
    let scroll = widgets::column_page(&column, 640);
    let page = widgets::page("", &header, &scroll);
    page.set_tag(Some(TAG));
    let refresh = {
        let ctx = Rc::downgrade(ctx);
        let page = page.downgrade();
        let column = column.downgrade();
        move || {
            let (Some(ctx), Some(page), Some(column)) =
                (ctx.upgrade(), page.upgrade(), column.upgrade())
            else {
                return;
            };
            let info = Info::of(&ctx.hub.known.borrow(), &subject);
            while let Some(child) = column.first_child() {
                column.remove(&child);
            }
            match info {
                Some(info) => {
                    page.set_title(&info.name());
                    fill(&ctx, &column, &info);
                }
                None => column.append(&widgets::haze("This source is no longer here.", "")),
            }
        }
    };
    refresh();
    let listening = ctx.hub.listen(move |event| {
        if matches!(event, Event::Providers(_) | Event::Stores(_) | Event::Settings(_)) {
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

fn fill(ctx: &Rc<Ctx>, column: &gtk::Box, info: &Info) {
    let name = info.name();
    let title = gtk::Label::new(Some(&name));
    title.add_css_class("title-2");
    title.set_xalign(0.0);
    title.set_wrap(true);
    column.append(&title);
    let origin = widgets::haze(&info.origin(), "");
    origin.set_margin_top(2);
    column.append(&origin);

    if let Some(row) = &info.row {
        let line = match Standing::of(row) {
            Standing::Failing => Some(match &row.last_error {
                Some(error) => format!("Not answering. {}", capitalized(error)),
                None => "Not answering.".into(),
            }),
            Standing::HeldBack => Some("Held back until the connection check passes.".into()),
            Standing::Off => Some("Disabled. It keeps its place and is not tried.".into()),
            _ => None,
        };
        if let Some(line) = line {
            let failing = Standing::of(row) == Standing::Failing;
            let mark = if failing { "vermilion" } else { "ring" };
            let state = widgets::standing(&line, mark);
            state.set_halign(gtk::Align::Start);
            state.set_margin_top(14);
            if let Some(label) = state.last_child().and_downcast::<gtk::Label>() {
                label.set_wrap(true);
                label.set_xalign(0.0);
                if failing {
                    label.add_css_class("problem");
                }
            }
            column.append(&state);
        }
    }

    let buttons = adw::WrapBox::builder().child_spacing(8).line_spacing(8).build();
    buttons.set_margin_top(16);
    let button = |label: &str| {
        let button = gtk::Button::with_label(label);
        button.add_css_class("pill");
        buttons.append(&button);
        button
    };
    let torrent = info.row.as_ref().is_some_and(|r| r.kind == "torrent");
    if torrent {
        let on = ctx
            .hub
            .known
            .borrow()
            .setting(SettingKey::Torrents)
            .is_some_and(|r| r.value == "on");
        if on {
            let off = button("Disable");
            let hub = Rc::downgrade(&ctx.hub);
            off.connect_clicked(move |_| {
                if let Some(hub) = hub.upgrade() {
                    hub.send(Command::SetSetting {
                        key: SettingKey::Torrents,
                        value: SettingValue::Bool(false),
                    });
                }
            });
        }
        let settings = button("Open Sources Settings");
        settings.set_action_name(Some("win.settings-pane"));
        settings.set_action_target_value(Some(&"sources".to_variant()));
        column.append(&buttons);
        let note = widgets::haze(
            "Torrents are part of anistream, not a plugin. What protects their traffic, the \
             connection check and seeding are set in Settings, under Sources.",
            "",
        );
        note.set_margin_top(12);
        column.append(&note);
        return;
    }

    // Update, Install or Replace, as the store offers it.
    if let Some((store, plugin)) = &info.listing {
        let live = store.state == StoreState::Ready;
        let (label, amber) = match &plugin.status {
            StorePluginStatus::Update { .. } => {
                (Some(format!("Update to {}", plugin.version)), true)
            }
            StorePluginStatus::Available if info.row.is_none() => {
                (Some("Install".into()), false)
            }
            StorePluginStatus::AddedByHand => (Some("Replace…".into()), false),
            _ => (None, false),
        };
        if let Some(label) = label {
            let action = button(&label);
            if amber {
                action.add_css_class("suggested-action");
            }
            action.set_sensitive(live);
            let ctx = Rc::clone(ctx);
            let (store, plugin) = (store.clone(), plugin.clone());
            action.connect_clicked(move |_| install(&ctx, &store, &plugin));
        }
    }
    if let Some(row) = &info.row {
        if Standing::of(row) == Standing::Failing {
            let again = button("Check Again");
            let hub = Rc::downgrade(&ctx.hub);
            again.connect_clicked(move |_| {
                if let Some(hub) = hub.upgrade() {
                    hub.send(Command::CheckProviders);
                }
            });
        }
        let toggle = button(if row.disabled { "Enable" } else { "Disable" });
        let (ctx2, id) = (Rc::clone(ctx), row.id.clone());
        toggle.connect_clicked(move |_| self::toggle(&ctx2, &id));
        let remove = button("Remove…");
        let (ctx2, id) = (Rc::clone(ctx), row.id.clone());
        remove.connect_clicked(move |_| confirm_remove(&ctx2, &id));
    }
    if buttons.first_child().is_some() {
        column.append(&buttons);
    }

    let facts = widgets::boxed_list();
    facts.set_margin_top(18);
    let fact = |title: &str, value: &str| {
        let row = adw::ActionRow::builder().title(title).subtitle(value).build();
        row.add_css_class("property");
        row.set_subtitle_selectable(true);
        facts.append(&row);
        row
    };
    let version = match (&info.row, &info.listing) {
        (_, Some((_, plugin))) => {
            let installed = match &plugin.status {
                StorePluginStatus::Update { from, .. } => from.clone(),
                _ => plugin.version.clone(),
            };
            match &plugin.status {
                StorePluginStatus::Update { .. } => {
                    format!("{installed}, {} available", plugin.version)
                }
                _ => installed,
            }
        }
        (Some(row), None) => row.version.clone().unwrap_or_else(|| "Not from a store".into()),
        (None, None) => String::new(),
    };
    fact("Version", &version);
    if let Some(row) = &info.row {
        let time = if Standing::of(row) == Standing::Failing {
            "No answer".into()
        } else if let Some(ms) = row.latency_ms {
            format!("{ms} ms")
        } else {
            "Not checked yet".into()
        };
        fact("Response time", &time);
    }
    let hosts = info.hosts();
    let contacts = fact(
        "Contacts",
        &if hosts.is_empty() { "Listed by its store".into() } else { hosts.join("\n") },
    );
    contacts.set_subtitle_lines(0);
    column.append(&facts);
    let note = widgets::haze(
        &format!("{name} can contact only these hosts and cannot read your files."),
        "caption",
    );
    note.set_margin_top(8);
    column.append(&note);
}

fn capitalized(text: &str) -> String {
    let mut chars = text.chars();
    chars.next().map(|c| c.to_uppercase().chain(chars).collect()).unwrap_or_default()
}

/// Switch the plugin `id` off, or back on.
pub fn toggle(ctx: &Rc<Ctx>, id: &str) {
    let disabled = ctx.hub.known.borrow().providers.iter().any(|r| r.id == id && r.disabled);
    ctx.hub.send(Command::DisablePlugin { id: id.into(), disabled: !disabled });
}

/// Ask before removing the plugin `id`, then leave its page.
pub fn confirm_remove(ctx: &Rc<Ctx>, id: &str) {
    let name = ctx
        .hub
        .known
        .borrow()
        .providers
        .iter()
        .find(|r| r.id == id)
        .map(providers::name)
        .unwrap_or_else(|| id.into());
    let dialog = adw::AlertDialog::new(
        Some(&format!("Remove {name}?")),
        Some("It is deleted from this computer. A store can install it again."),
    );
    dialog.add_response("cancel", "Cancel");
    dialog.add_response("remove", "Remove");
    dialog.set_response_appearance("remove", adw::ResponseAppearance::Destructive);
    dialog.set_default_response(Some("cancel"));
    dialog.set_close_response("cancel");
    let weak = Rc::downgrade(ctx);
    let id = id.to_owned();
    dialog.connect_response(None, move |_, response| {
        let (Some(ctx), "remove") = (weak.upgrade(), response) else { return };
        ctx.hub.send(Command::RemovePlugin { plugin: id.clone() });
        if let Some(nav) = ctx.nav()
            && nav.visible_page().and_then(|p| p.tag()).as_deref() == Some(TAG)
        {
            nav.pop();
        }
    });
    dialog.present(Some(&ctx.window));
}

/// Install `plugin` from `store`; an update or a replacement asks first, naming the hosts it
/// adds.
pub fn install(ctx: &Rc<Ctx>, store: &StoreRow, plugin: &StorePluginRow) {
    let send = {
        let hub = Rc::downgrade(&ctx.hub);
        let (store, plugin) = (store.id.clone(), plugin.id.clone());
        move || {
            if let Some(hub) = hub.upgrade() {
                hub.send(Command::InstallPlugin {
                    store: store.clone(),
                    plugin: plugin.clone(),
                });
            }
        }
    };
    let (heading, body, verb) = match &plugin.status {
        StorePluginStatus::Update { from, new_hosts } => {
            let mut body =
                format!("Version {} from {} replaces {from}.", plugin.version, store.name);
            if let Some(line) = new_hosts_line(new_hosts) {
                body = format!("{body} {line}");
            }
            (format!("Update {}?", plugin.name), body, "Update")
        }
        StorePluginStatus::AddedByHand => (
            format!("Replace {}?", plugin.name),
            format!(
                "Version {} from {} replaces the copy added by hand. {}.",
                plugin.version,
                store.name,
                contacts_line(&plugin.hosts)
            ),
            "Replace",
        ),
        _ => {
            send();
            return;
        }
    };
    let dialog = adw::AlertDialog::new(Some(&heading), Some(&body));
    dialog.add_response("cancel", "Cancel");
    dialog.add_response("install", verb);
    dialog.set_response_appearance("install", adw::ResponseAppearance::Suggested);
    dialog.set_default_response(Some("install"));
    dialog.set_close_response("cancel");
    dialog.connect_response(None, move |_, response| {
        if response == "install" {
            send();
        }
    });
    dialog.present(Some(&ctx.window));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_update_names_the_hosts_it_adds() {
        let one = ["media2.example".to_owned()];
        assert_eq!(
            new_hosts_line(&one).as_deref(),
            Some("This update also contacts media2.example.")
        );
        let three = ["a.example", "b.example", "c.example"].map(String::from);
        assert_eq!(
            new_hosts_line(&three).as_deref(),
            Some("This update also contacts a.example and 2 more.")
        );
        assert_eq!(new_hosts_line(&[]), None);
        assert_eq!(contacts_line(&three), "Contacts a.example and 2 more");
        assert_eq!(capitalized("the site did not answer."), "The site did not answer.");
    }
}
