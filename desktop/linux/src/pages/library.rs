//! Library: the tracker's five lists, each a poster grid with its count, picked with an inline
//! view switcher under the header bar (a drop-down on narrow windows), the order in a header bar
//! menu. Signed out, Nagare and one action, Sign In to AniList.

use std::{cell::RefCell, collections::HashMap, rc::Rc};

use adw::prelude::*;
use anistream_model::{
    catalog::{ContentFor, ContentList, Entry, LibrarySegment},
    command::Command,
    event::Event,
};
use gtk::{gio, glib};

use crate::{
    nagare,
    states::{self, Shape, States},
    widgets::{self, PosterGrid},
    window::{Ctx, Size},
    say,
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
            Self::Score => entries.sort_by_key(|e| std::cmp::Reverse(e.score)),
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
    lists: Vec<(LibrarySegment, PosterGrid, adw::ViewStackPage, Rc<States>)>,
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
        let grid = PosterGrid::with_caption(&ctx.hub, say::progress_caption);
        let scroll = gtk::ScrolledWindow::builder()
            .hscrollbar_policy(gtk::PolicyType::Never)
            .child(&grid.widget)
            .build();
        let empty = states::plain(
            &format!("Nothing in {}", segment_title(segment)),
            "Titles you set to this status on AniList show here.",
        );
        let list = States::new(
            &ctx.hub,
            &scroll,
            Shape::Grid,
            &empty,
            &format!("Couldn't Load {}", segment_title(segment)),
        );
        list.on_retry({
            let hub = Rc::downgrade(&ctx.hub);
            let list = Rc::downgrade(&list);
            move || {
                if let (Some(hub), Some(list)) = (hub.upgrade(), list.upgrade()) {
                    list.loading(Some("loading your lists".into()));
                    hub.send(Command::LoadLibrary(segment));
                }
            }
        });
        let page =
            views.add_titled(&list.widget, Some(segment.label()), segment_title(segment));
        lists.push((segment, grid, page, list));
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

    let connect = gtk::Button::with_label("Sign In to AniList");
    connect.add_css_class("pill");
    connect.add_css_class("suggested-action");
    connect.set_halign(gtk::Align::Center);
    connect.set_action_name(Some("win.connect"));
    connect.set_action_target_value(Some(&"anilist".to_variant()));
    let signed_out =
        nagare::status_page("Library", "Your lists come from AniList.", Some(&connect));

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
    header.pack_start(&crate::window::tv_session_button(ctx));
    header.pack_end(&order_button);
    let navigation = widgets::content_page("Library", &header, &body);

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
            let (Some(page), Some(name)) = (page.upgrade(), arg.and_then(String::from_variant))
            else {
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
                ContentList::Loading => {
                    if let Some(list) = self.list(*segment) {
                        list.loading(Some("loading your lists".into()));
                    }
                }
                ContentList::Failed(why) => {
                    tracing::warn!(%why, list = segment.label(), "a list did not load");
                    if let Some(list) = self.list(*segment) {
                        list.failed(why);
                    }
                }
                _ => {
                    self.entries
                        .borrow_mut()
                        .insert(segment.label(), content.entries().to_vec());
                    self.show(*segment);
                }
            },
            // Titles are named another way now: the lists again, once they have been shown.
            Event::TitlesChanged(_) if !self.entries.borrow().is_empty() => self.load(),
            Event::Sync(_) => {
                let signed_in = self.ctx.hub.known.borrow().anilist().is_some();
                self.signed(signed_in);
            }
            // A status set here moves the title between lists.
            Event::Detail(entry) if entry.list_status.is_some() => {
                let moved = self.entries.borrow().iter().any(|(segment, list)| {
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
            if let Some(list) = self.list(segment) {
                list.loading(Some("loading your lists".into()));
            }
            self.ctx.hub.send(Command::LoadLibrary(segment));
        }
    }

    fn list(&self, segment: LibrarySegment) -> Option<&Rc<States>> {
        self.lists.iter().find(|(s, ..)| *s == segment).map(|(.., list)| list)
    }

    fn show(&self, segment: LibrarySegment) {
        let Some((_, grid, page, list)) = self.lists.iter().find(|(s, ..)| *s == segment)
        else {
            return;
        };
        let Some(mut entries) = self.entries.borrow().get(segment.label()).cloned() else {
            return;
        };
        self.order.borrow().sort(&mut entries);
        grid.set(&entries);
        page.set_title(Some(&format!("{} {}", segment_title(segment), entries.len())));
        if entries.is_empty() {
            list.empty();
        } else {
            list.content();
        }
    }
}
