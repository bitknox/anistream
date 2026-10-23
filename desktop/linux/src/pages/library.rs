//! Library: the tracker's five lists, each a poster grid with its count, picked with an inline
//! view switcher under the header bar (a drop-down on narrow windows), the order in a header bar
//! menu. Signed out, Nagare and one action, Connect AniList.

use std::{cell::RefCell, collections::HashMap, rc::Rc};

use anistream_model::{
    catalog::{ContentFor, ContentList, Entry, LibrarySegment},
    command::Command,
    event::Event,
};
use adw::prelude::*;
use gtk::{gio, glib};

use crate::{
    nagare,
    widgets::{self, PosterGrid},
    window::{Ctx, Size},
};

/// How a list is ordered.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Order {
    Updated,
    Title,
    Score,
}

impl Order {
    fn named(name: &str) -> Self {
        match name {
            "title" => Self::Title,
            "score" => Self::Score,
            _ => Self::Updated,
        }
    }

    fn sort(self, entries: &mut [Entry]) {
        match self {
            // The tracker's own order is last updated first.
            Self::Updated => {}
            Self::Title => entries.sort_by_key(|e| e.title.to_lowercase()),
            Self::Score => entries.sort_by(|a, b| b.score.cmp(&a.score)),
        }
    }
}

fn segment_title(segment: LibrarySegment) -> &'static str {
    match segment {
        LibrarySegment::Watching => "Watching",
        LibrarySegment::Planning => "Planning",
        LibrarySegment::Completed => "Completed",
        LibrarySegment::Paused => "Paused",
        LibrarySegment::Dropped => "Dropped",
    }
}

struct Page {
    ctx: Rc<Ctx>,
    lists: Vec<(LibrarySegment, PosterGrid, adw::ViewStackPage, gtk::Stack)>,
    entries: RefCell<HashMap<&'static str, Vec<Entry>>>,
    order: RefCell<Order>,
    body: gtk::Stack,
    signed_in: RefCell<Option<bool>>,
}

pub fn build(ctx: &Rc<Ctx>) -> adw::NavigationPage {
    let views = adw::ViewStack::new();
    views.set_vexpand(true);
    let mut lists = Vec::new();
    for segment in LibrarySegment::ALL {
        let grid = PosterGrid::new(&ctx.hub);
        let scroll = gtk::ScrolledWindow::builder()
            .hscrollbar_policy(gtk::PolicyType::Never)
            .child(&grid.widget)
            .build();
        let empty = nagare::status_page(
            &format!("Nothing in {}", segment_title(segment)),
            "Titles you set to this status on your tracker show here.",
            None,
        );
        let loading = adw::Spinner::new();
        loading.set_size_request(32, 32);
        loading.set_halign(gtk::Align::Center);
        loading.set_valign(gtk::Align::Center);
        let stack = gtk::Stack::new();
        stack.add_named(&loading, Some("loading"));
        stack.add_named(&scroll, Some("grid"));
        stack.add_named(&empty, Some("empty"));
        let page = views.add_titled(&stack, Some(segment.label()), segment_title(segment));
        lists.push((segment, grid, page, stack));
    }

    let switcher = adw::InlineViewSwitcher::new();
    switcher.set_stack(Some(&views));
    switcher.set_halign(gtk::Align::Center);
    switcher.add_css_class("round");
    let names: Vec<&str> = LibrarySegment::ALL.iter().map(|s| segment_title(*s)).collect();
    let picker = gtk::DropDown::from_strings(&names);
    picker.set_halign(gtk::Align::Start);
    picker.connect_selected_notify({
        let views = views.clone();
        move |picker| {
            if let Some(segment) = LibrarySegment::ALL.get(picker.selected() as usize) {
                views.set_visible_child_name(segment.label());
            }
        }
    });
    views.connect_visible_child_name_notify({
        let picker = picker.clone();
        move |views| {
            let name = views.visible_child_name().unwrap_or_default();
            if let Some(at) = LibrarySegment::ALL.iter().position(|s| s.label() == name) {
                picker.set_selected(at as u32);
            }
        }
    });
    let bar = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    bar.set_margin_top(12);
    bar.set_margin_bottom(6);
    bar.set_margin_start(18);
    bar.set_margin_end(18);
    bar.append(&switcher);
    bar.append(&picker);
    switcher.set_hexpand(true);
    ctx.on_size({
        let (switcher, picker) = (switcher.clone(), picker.clone());
        move |size| {
            switcher.set_visible(size != Size::Narrow);
            picker.set_visible(size == Size::Narrow);
        }
    });

    let lists_view = gtk::Box::new(gtk::Orientation::Vertical, 0);
    lists_view.append(&bar);
    lists_view.append(&views);

    let connect = gtk::Button::with_label("Connect AniList");
    connect.add_css_class("pill");
    connect.add_css_class("suggested-action");
    connect.set_halign(gtk::Align::Center);
    connect.set_action_name(Some("win.connect"));
    connect.set_action_target_value(Some(&"anilist".to_variant()));
    let signed_out = nagare::status_page("Library", "Your lists come from your tracker.", Some(&connect));

    let body = gtk::Stack::new();
    body.add_named(&lists_view, Some("lists"));
    body.add_named(&signed_out, Some("signed-out"));

    let order_menu = gio::Menu::new();
    for (label, name) in [("Last Updated", "updated"), ("Title", "title"), ("Score", "score")] {
        order_menu.append(Some(label), Some(&format!("library.order::{name}")));
    }
    let menu = gio::Menu::new();
    menu.append_section(Some("Order"), &order_menu);
    let order_button = gtk::MenuButton::builder()
        .icon_name("view-sort-descending-symbolic")
        .tooltip_text("Order")
        .menu_model(&menu)
        .build();
    let header = adw::HeaderBar::new();
    header.pack_start(&widgets::sidebar_button(&ctx.split));
    header.pack_end(&order_button);
    let navigation = widgets::page("Library", &header, &body);

    let page = Rc::new(Page {
        ctx: Rc::clone(ctx),
        lists,
        entries: RefCell::default(),
        order: RefCell::new(Order::Updated),
        body,
        signed_in: RefCell::default(),
    });

    let order = gio::SimpleAction::new_stateful(
        "order",
        Some(glib::VariantTy::STRING),
        &"updated".to_variant(),
    );
    order.connect_activate({
        let page = Rc::downgrade(&page);
        move |action, arg| {
            let (Some(page), Some(name)) = (page.upgrade(), arg.and_then(String::from_variant)) else {
                return;
            };
            action.set_state(&name.to_variant());
            page.order.replace(Order::named(&name));
            for segment in LibrarySegment::ALL {
                page.show(segment);
            }
        }
    });
    let group = gio::SimpleActionGroup::new();
    group.add_action(&order);
    navigation.insert_action_group("library", Some(&group));

    let weak = Rc::downgrade(&page);
    ctx.hub.listen(move |event| {
        if let Some(page) = weak.upgrade() {
            page.heard(event);
        }
    });
    page.signed(ctx.hub.known.borrow().anilist().is_some());
    widgets::keep(&navigation, page);
    navigation
}

impl Page {
    fn heard(&self, event: &Event) {
        match event {
            Event::Content { view: ContentFor::Library(segment), content } => match content {
                ContentList::Loading => {}
                ContentList::Failed(why) => {
                    tracing::warn!(%why, list = segment.label(), "a list did not load");
                    self.entries.borrow_mut().insert(segment.label(), Vec::new());
                    self.show(*segment);
                }
                _ => {
                    self.entries.borrow_mut().insert(segment.label(), content.entries().to_vec());
                    self.show(*segment);
                }
            },
            Event::Sync(_) => {
                let signed_in = self.ctx.hub.known.borrow().anilist().is_some();
                self.signed(signed_in);
            }
            // A status set here moves the title between lists.
            Event::Detail(entry) if entry.list_status.is_some() => {
                let moved = self
                    .entries
                    .borrow()
                    .iter()
                    .any(|(segment, list)| {
                        list.iter().any(|e| e.id == entry.id)
                            && Some(*segment) != entry.list_status.map(LibrarySegment::label)
                    });
                if moved {
                    self.load();
                }
            }
            _ => {}
        }
    }

    fn signed(&self, signed_in: bool) {
        if self.signed_in.replace(Some(signed_in)) == Some(signed_in) {
            return;
        }
        self.body.set_visible_child_name(if signed_in { "lists" } else { "signed-out" });
        if signed_in {
            self.load();
        }
    }

    fn load(&self) {
        for segment in LibrarySegment::ALL {
            self.ctx.hub.send(Command::LoadLibrary(segment));
        }
    }

    fn show(&self, segment: LibrarySegment) {
        let Some((_, grid, page, stack)) = self.lists.iter().find(|(s, ..)| *s == segment) else {
            return;
        };
        let Some(mut entries) = self.entries.borrow().get(segment.label()).cloned() else { return };
        self.order.borrow().sort(&mut entries);
        grid.set(&entries);
        page.set_title(Some(&format!("{} {}", segment_title(segment), entries.len())));
        stack.set_visible_child_name(if entries.is_empty() { "empty" } else { "grid" });
    }
}
