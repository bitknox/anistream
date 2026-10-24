//! Providers: the sources in the order failover tries them, as a boxed list. Each row has a
//! handle to drag it by, its store in one quiet line, and a word only for a problem, an update
//! or off. Move Up and Move Down are in each row's menu; a row opens its source's page.

use std::{cell::Cell, rc::Rc};

use adw::prelude::*;
use anistream_core::settings::SettingValue;
use anistream_model::{
    command::Command,
    event::Event,
    settings::SettingKey,
    sources::{ProviderOrigin, ProviderRow, order_with_source_moved},
};
use gtk::{gdk, gio, glib};

use crate::{
    nagare,
    pages::plugin,
    widgets,
    window::{Ctx, Size},
};

/// How a source stands, as the list says it beside the name.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Standing {
    Working,
    Slow,
    Failing,
    HeldBack,
    Off,
}

impl Standing {
    pub fn of(row: &ProviderRow) -> Self {
        if row.disabled {
            Self::Off
        } else if row.held_back {
            Self::HeldBack
        } else if !row.usable || row.state == "down" {
            Self::Failing
        } else if row.state == "degraded" {
            Self::Slow
        } else {
            Self::Working
        }
    }
}

/// The name a source goes by.
pub fn name(row: &ProviderRow) -> String {
    match row.kind.as_str() {
        "torrent" => "Torrents".into(),
        "remote" => "Remote".into(),
        _ if row.name.is_empty() => row.id.clone(),
        _ => row.name.clone(),
    }
}

/// Where a source came from, in one quiet line: its store, or how it got here.
pub fn origin(row: &ProviderRow) -> String {
    match (&row.origin, row.kind.as_str()) {
        (_, "remote") => "Set in the config file".into(),
        (ProviderOrigin::BuiltIn, _) => "Built in".into(),
        (ProviderOrigin::Store { name: Some(name), .. }, _) => name.clone(),
        (ProviderOrigin::Store { name: None, .. }, _) => "From a store you left".into(),
        (ProviderOrigin::ByHand, _) => "Added by hand".into(),
    }
}

/// The word beside a source, and its mark: only for a problem, an update or off.
pub fn standing_words(row: &ProviderRow) -> Option<(&'static str, &'static str)> {
    match Standing::of(row) {
        Standing::Failing => Some(("Not answering", "vermilion")),
        Standing::HeldBack => Some(("Held back", "ring")),
        Standing::Off => Some(("Off", "ring")),
        Standing::Slow => Some(("Slow", "haze")),
        Standing::Working if row.update.is_some() => Some(("Update available", "water")),
        Standing::Working => None,
    }
}

/// `order` with `id` moved to where the row at `target` is on screen, one place at a time, so a
/// drag writes what the same number of Move Up or Move Down would.
pub fn moved_to(
    order: &[String],
    rows: &[ProviderRow],
    id: &str,
    target: usize,
) -> Option<Vec<String>> {
    let from = rows.iter().position(|r| r.id == id)?;
    if from == target || target >= rows.len() {
        return None;
    }
    let step = if target < from { -1 } else { 1 };
    let mut order = order.to_vec();
    for _ in 0..from.abs_diff(target) {
        match order_with_source_moved(&order, rows, id, step) {
            Some(next) => order = next,
            None => break,
        }
    }
    Some(order)
}

/// What a row's action does with the source it names.
type OnId = Box<dyn Fn(&Rc<Page>, String)>;

struct Page {
    ctx: Rc<Ctx>,
    body: gtk::Stack,
    list: gtk::ListBox,
    shown: Cell<bool>,
}

pub fn build(ctx: &Rc<Ctx>) -> adw::NavigationPage {
    let check = gtk::Button::from_icon_name("view-refresh-symbolic");
    check.set_tooltip_text(Some("Check Again"));
    check.set_action_name(Some("providers.check"));
    let header = adw::HeaderBar::new();
    header.pack_start(&widgets::sidebar_button(&ctx.split));
    header.pack_end(&check);

    let list = widgets::boxed_list();
    let column = gtk::Box::new(gtk::Orientation::Vertical, 12);
    column.append(&widgets::heading("Source Order"));
    column.append(&list);
    let note = widgets::haze(
        "Sources are tried top to bottom. Drag one by its handle to change the order.",
        "caption",
    );
    column.append(&note);
    let legal = widgets::haze("anistream hosts no content.", "caption");
    legal.set_margin_top(12);
    column.append(&legal);

    let join = gtk::Button::with_label("Join Store…");
    join.add_css_class("pill");
    join.set_action_name(Some("win.join-store"));
    let empty = nagare::status_page(
        "No Sources Yet",
        "Sources come from plugin stores. Join one with the invite link its owner sends you.",
        Some(&join),
    );
    let body = gtk::Stack::new();
    body.add_named(&widgets::column_page(&column, 720), Some("list"));
    body.add_named(&empty, Some("empty"));
    let navigation = widgets::page("Providers", &header, &body);

    let page = Rc::new(Page { ctx: Rc::clone(ctx), body, list, shown: Cell::new(false) });
    page.actions(&navigation);
    let weak = Rc::downgrade(&page);
    ctx.hub.listen(move |event| {
        if let Some(page) = weak.upgrade()
            && page.shown.get()
            && matches!(event, Event::Providers(_) | Event::Stores(_) | Event::Settings(_))
        {
            page.show();
        }
    });
    ctx.on_size({
        let weak = Rc::downgrade(&page);
        move |_| {
            if let Some(page) = weak.upgrade()
                && page.shown.get()
            {
                page.show();
            }
        }
    });
    navigation.connect_map({
        let weak = Rc::downgrade(&page);
        move |_| {
            if let Some(page) = weak.upgrade() {
                page.shown.set(true);
                page.ctx.hub.send(Command::CheckProviders);
                page.ctx.hub.send(Command::LoadStores);
                page.show();
            }
        }
    });
    widgets::keep(&navigation, page);
    navigation
}

impl Page {
    fn actions(self: &Rc<Self>, navigation: &adw::NavigationPage) {
        let group = gio::SimpleActionGroup::new();
        let add = |name: &str, f: OnId| {
            let action = gio::SimpleAction::new(name, Some(glib::VariantTy::STRING));
            let weak = Rc::downgrade(self);
            action.connect_activate(move |_, target| {
                if let (Some(page), Some(id)) =
                    (weak.upgrade(), target.and_then(String::from_variant))
                {
                    f(&page, id);
                }
            });
            group.add_action(&action);
        };
        add("open", Box::new(|page, id| plugin::push_provider(&page.ctx, &id)));
        add("up", Box::new(|page, id| page.step(&id, -1)));
        add("down", Box::new(|page, id| page.step(&id, 1)));
        add("toggle", Box::new(|page, id| plugin::toggle(&page.ctx, &id)));
        add("remove", Box::new(|page, id| plugin::confirm_remove(&page.ctx, &id)));
        let check = gio::SimpleAction::new("check", None);
        let hub = Rc::downgrade(&self.ctx.hub);
        check.connect_activate(move |_, _| {
            if let Some(hub) = hub.upgrade() {
                hub.send(Command::CheckProviders);
            }
        });
        group.add_action(&check);
        navigation.insert_action_group("providers", Some(&group));
    }

    fn step(&self, id: &str, step: isize) {
        let (order, rows) = {
            let known = self.ctx.hub.known.borrow();
            (known.source_order(), known.ordered_providers())
        };
        if let Some(order) = order_with_source_moved(&order, &rows, id, step) {
            self.write(order);
        }
    }

    fn move_to(&self, id: &str, target: usize) {
        let (order, rows) = {
            let known = self.ctx.hub.known.borrow();
            (known.source_order(), known.ordered_providers())
        };
        if let Some(order) = moved_to(&order, &rows, id, target) {
            self.write(order);
        }
    }

    fn write(&self, order: Vec<String>) {
        self.ctx.hub.send(Command::SetSetting {
            key: SettingKey::SourceOrder,
            value: SettingValue::Strings(order),
        });
    }

    fn show(self: &Rc<Self>) {
        let rows = self.ctx.hub.known.borrow().ordered_providers();
        while let Some(child) = self.list.first_child() {
            self.list.remove(&child);
        }
        self.body.set_visible_child_name(if rows.is_empty() { "empty" } else { "list" });
        for (at, row) in rows.iter().enumerate() {
            self.list.append(&self.row(row, at, rows.len()));
        }
    }

    fn row(self: &Rc<Self>, row: &ProviderRow, at: usize, count: usize) -> adw::ActionRow {
        let line = adw::ActionRow::builder()
            .title(name(row))
            .subtitle(origin(row))
            .activatable(true)
            .action_name("providers.open")
            .action_target(&row.id.to_variant())
            .build();
        line.set_title_lines(1);
        line.set_subtitle_lines(1);

        let handle = gtk::Image::from_icon_name("list-drag-handle-symbolic");
        handle.add_css_class("dim-label");
        handle.set_tooltip_text(Some("Drag to Change the Order"));
        handle.set_cursor_from_name(Some("grab"));
        line.add_prefix(&handle);
        let drag = gtk::DragSource::new();
        drag.set_actions(gdk::DragAction::MOVE);
        let id = row.id.clone();
        drag.connect_prepare(move |_, _, _| {
            Some(gdk::ContentProvider::for_value(&id.to_value()))
        });
        let weak_line = line.downgrade();
        drag.connect_drag_begin(move |source, _| {
            if let Some(line) = weak_line.upgrade() {
                let paintable = gtk::WidgetPaintable::new(Some(&line));
                source.set_icon(Some(&paintable), 24, 24);
            }
        });
        handle.add_controller(drag);
        let drop = gtk::DropTarget::new(String::static_type(), gdk::DragAction::MOVE);
        let weak = Rc::downgrade(self);
        drop.connect_drop(move |_, value, _, _| {
            let (Some(page), Ok(id)) = (weak.upgrade(), value.get::<String>()) else {
                return false;
            };
            page.move_to(&id, at);
            true
        });
        line.add_controller(drop);

        if let Some((words, mark)) = standing_words(row) {
            if self.ctx.size() == Size::Narrow {
                // No room beside the name: the mark stays, the words join the quiet line.
                line.set_subtitle(&format!("{words} · {}", origin(row)));
                let dot = widgets::standing("", mark);
                dot.set_tooltip_text(Some(words));
                line.add_suffix(&dot);
            } else {
                line.add_suffix(&widgets::standing(words, mark));
            }
        }
        let menu = gio::Menu::new();
        let order = gio::Menu::new();
        if at > 0 {
            order.append_item(&widgets::menu_item("Move Up", "providers.up", &row.id));
        }
        if at + 1 < count {
            order.append_item(&widgets::menu_item("Move Down", "providers.down", &row.id));
        }
        menu.append_section(None, &order);
        if row.kind == "plugin" {
            let more = gio::Menu::new();
            let toggle = if row.disabled { "Enable" } else { "Disable" };
            more.append_item(&widgets::menu_item(toggle, "providers.toggle", &row.id));
            more.append_item(&widgets::menu_item("Remove…", "providers.remove", &row.id));
            menu.append_section(None, &more);
        }
        let button = gtk::MenuButton::builder()
            .icon_name("view-more-symbolic")
            .tooltip_text("More")
            .menu_model(&menu)
            .valign(gtk::Align::Center)
            .build();
        button.add_css_class("flat");
        line.add_suffix(&button);
        line.add_suffix(&widgets::chevron());
        line
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row(id: &str, kind: &str) -> ProviderRow {
        ProviderRow {
            id: id.into(),
            kind: kind.into(),
            state: "ready".into(),
            usable: true,
            name: id.into(),
            ..ProviderRow::default()
        }
    }

    #[test]
    fn a_source_speaks_only_for_a_problem_an_update_or_off() {
        let ready = row("a", "plugin");
        assert_eq!(standing_words(&ready), None);
        let down = ProviderRow { state: "down".into(), usable: false, ..row("a", "plugin") };
        assert_eq!(standing_words(&down), Some(("Not answering", "vermilion")));
        let off = ProviderRow { disabled: true, ..down.clone() };
        assert_eq!(standing_words(&off), Some(("Off", "ring")));
        let update = ProviderRow {
            update: Some(anistream_model::sources::ProviderUpdate {
                version: "2".into(),
                new_hosts: vec![],
            }),
            ..row("a", "plugin")
        };
        assert_eq!(standing_words(&update), Some(("Update available", "water")));
        assert_eq!(name(&row("torrent", "torrent")), "Torrents");
        assert_eq!(origin(&row("torrent", "torrent")), "Built in");
    }

    #[test]
    fn a_drag_moves_as_far_as_the_same_presses_would() {
        let rows = [row("a", "plugin"), row("b", "plugin"), row("torrent", "torrent")];
        let order: Vec<String> = ["plugins", "torrent"].map(String::from).to_vec();
        let moved = moved_to(&order, &rows, "torrent", 0).unwrap();
        let shown = crate::hub::sorted(&rows, &moved);
        assert_eq!(
            shown.iter().map(|r| r.id.as_str()).collect::<Vec<_>>(),
            ["torrent", "a", "b"]
        );
        assert_eq!(moved_to(&order, &rows, "a", 0), None, "already there");
    }
}
